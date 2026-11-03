//! xHCI USB host controller driver (polling, no interrupts) for boot-protocol keyboards.
//!
//! `init` finds the controller on the PCI bus, takes it over from the firmware, resets and starts it,
//! then looks at every connected port: reset the port, enable a device slot, give the device an address,
//! read its descriptors, and when it is a boot keyboard switch it to the boot protocol and start polling
//! its interrupt endpoint. `poll_key` hands the keys to the shell. Register decoding (`xhcireg`),
//! rings (`xhcitrb`), contexts (`xhcictx`), descriptors (`usbdesc`) and key translation (`hidkbd`) are pure
//! and host-tested; this file only does the hardware access.
//!
//! Every wait has a timeout, and any failure leaves the controller unused: the PS/2 keyboard path does
//! not depend on this driver. Not handled: hubs, hot plug (devices must be present at boot), more than
//! one keyboard.
//!
//! Global state: `XHCI` follows rule A in `docs/design/globals.md`: only boot code (`init`) and the shell
//! task touch it, never an interrupt handler.

use core::ptr::read_volatile;

use crate::global::Global;
use crate::hidkbd::HidKeyboard;
use crate::keymap::Key;
use crate::usbdesc;
use crate::xhcictx::{self, Endpoint};
use crate::xhcireg::{self as reg, Caps};
use crate::xhcitrb::{EventRing, ProducerRing, Trb, CC_SHORT_PACKET, CC_SUCCESS, TRB_COMMAND_COMPLETION, TRB_TRANSFER_EVENT};
use crate::mmio::{dma_page, rd32, wait_until, wr32, wr64, PAGE_BYTES};
use crate::{arch, log, paging, pci};

/// TRBs per ring: one 4 KiB frame holds 256 of 16 bytes.
const RING_TRBS: usize = 256;
/// Most device slots this driver asks the controller for.
const MAX_SLOTS: u8 = 32;
/// Longest configuration descriptor that is read (it must fit the one-page buffer).
const MAX_CONFIG_DESC: u16 = 1024;

// ---- register and memory access (shared helpers are in `mmio.rs`) ----

/// The bytes of a DMA page.
///
/// # Safety
/// `page` must come from `dma_page`, and the controller must not be writing it while the slice is used
/// (true for input contexts and descriptor buffers between commands).
unsafe fn page_bytes(page: u64) -> &'static mut [u8] {
    // SAFETY: the caller's contract: a live, never-freed 4 KiB frame.
    unsafe { core::slice::from_raw_parts_mut(page as *mut u8, PAGE_BYTES) }
}

/// A transfer or command ring in a fresh DMA page.
fn new_ring(ac64: bool) -> Result<(u64, ProducerRing<'static>), &'static str> {
    let phys = dma_page(ac64)?;
    // SAFETY: a zeroed frame nobody else uses; it is never freed, so the slice is valid for the rest of the run.
    let mem: &'static mut [Trb] = unsafe { core::slice::from_raw_parts_mut(phys as *mut Trb, RING_TRBS) };
    let ring = ProducerRing::new(mem, phys).ok_or("ring too small")?;
    Ok((phys, ring))
}

// ---- the controller ----

/// Keys waiting for the shell (a report can carry several presses).
struct KeyQueue {
    items: [Option<Key>; 16],
    head: usize,
    len: usize,
}

impl KeyQueue {
    const fn new() -> Self {
        Self { items: [None; 16], head: 0, len: 0 }
    }

    fn push(&mut self, key: Key) {
        if self.len < self.items.len() {
            let at = (self.head + self.len) % self.items.len();
            self.items[at] = Some(key);
            self.len += 1;
        }
    }

    fn pop(&mut self) -> Option<Key> {
        if self.len == 0 {
            return None;
        }
        let key = self.items[self.head].take();
        self.head = (self.head + 1) % self.items.len();
        self.len -= 1;
        key
    }
}

struct Keyboard {
    port: u8,
    slot: u8,
    dci: u8,
    ring: ProducerRing<'static>,
    /// Physical address of the 8-byte report buffer.
    report: u64,
    hid: HidKeyboard,
    queue: KeyQueue,
    errors: u8,
}

/// A device that has a slot and an address.
struct Device {
    slot: u8,
    ep0: ProducerRing<'static>,
    /// Physical address of the page used for descriptors.
    buf: u64,
}

pub struct Xhci {
    vendor: u16,
    device: u16,
    caps: Caps,
    slots: u8,
    /// Operational registers, doorbells and runtime registers.
    op: u64,
    db: u64,
    rt: u64,
    dcbaa: u64,
    handoff: &'static str,
    cmd: ProducerRing<'static>,
    events: EventRing,
    event_mem: u64,
    keyboard: Option<Keyboard>,
}

static XHCI: Global<Option<Xhci>> = Global::new(None);

/// Asks the firmware to hand the controller over (USB legacy support capability).
fn take_over_from_firmware(base: u64, caps: &Caps) -> &'static str {
    let mut at = u64::from(caps.xecp_dwords) * 4;
    if at == 0 {
        return "no extended capabilities";
    }
    for _ in 0..64 {
        if at >= 0x1_0000 {
            break; // outside the window that was mapped
        }
        let header = rd32(base + at);
        let (id, next) = reg::xecp_header(header);
        if id == reg::LEGACY_CAP_ID {
            if header & reg::LEGACY_BIOS_OWNED == 0 {
                return "firmware does not own it";
            }
            wr32(base + at, header | reg::LEGACY_OS_OWNED);
            let released = wait_until(1000, || rd32(base + at) & reg::LEGACY_BIOS_OWNED == 0);
            return if released { "taken over from the firmware" } else { "firmware did not let go, continuing" };
        }
        if next == 0 {
            break;
        }
        at += u64::from(next) * 4;
    }
    "no legacy support capability"
}

fn bring_up(d: &pci::Device) -> Result<Xhci, &'static str> {
    // Memory space and bus mastering on.
    let command = pci::config_read(d.bus, d.dev, d.func, 4);
    pci::config_write(d.bus, d.dev, d.func, 4, (command & 0xFFFF) | 0x6);
    let base = pci::memory_bar(d, 0).ok_or("BAR 0 is not a memory BAR")?;
    if !(paging::map_mmio_uc(base) && paging::map_mmio_uc(base + 0xFFFF)) {
        return Err("cannot map the registers");
    }
    let caps = Caps::decode(rd32(base), rd32(base + 4), rd32(base + 8), rd32(base + 0x10), rd32(base + 0x14), rd32(base + 0x18));
    if caps.cap_length < 0x20 || caps.max_slots == 0 || caps.max_ports == 0 || caps.max_ports > 127 {
        return Err("capability registers look wrong");
    }
    let op = base + u64::from(caps.cap_length);
    let rt = base + u64::from(caps.runtime_offset);
    let db = base + u64::from(caps.doorbell_offset);
    let handoff = take_over_from_firmware(base, &caps);

    // Stop the controller if it runs, then reset it.
    if rd32(op) & 1 != 0 {
        wr32(op, rd32(op) & !1);
        if !wait_until(100, || rd32(op + 4) & 1 != 0) {
            return Err("controller would not stop");
        }
    }
    wr32(op, rd32(op) | 2);
    if !wait_until(1000, || rd32(op) & 2 == 0 && rd32(op + 4) & (1 << 11) == 0) {
        return Err("controller reset did not finish");
    }

    let slots = caps.max_slots.min(MAX_SLOTS);
    wr32(op + 0x38, u32::from(slots));

    // Device context base address array, with the scratchpad buffers the controller asked for.
    let dcbaa = dma_page(caps.ac64)?;
    if caps.scratchpads > 0 {
        let n = usize::from(caps.scratchpads);
        if n > PAGE_BYTES / 8 {
            return Err("too many scratchpad buffers");
        }
        let array = dma_page(caps.ac64)?;
        for i in 0..n {
            wr64(array + 8 * i as u64, dma_page(caps.ac64)?);
        }
        wr64(dcbaa, array);
    }
    wr64(op + 0x30, dcbaa);

    // Command ring (ring cycle state 1).
    let (cmd_phys, cmd) = new_ring(caps.ac64)?;
    wr64(op + 0x18, cmd_phys | 1);

    // Event ring with a one-entry segment table on interrupter 0.
    let event_mem = dma_page(caps.ac64)?;
    let table = dma_page(caps.ac64)?;
    wr64(table, event_mem);
    wr32(table + 8, RING_TRBS as u32);
    wr32(rt + 0x28, 1); // ERSTSZ
    wr64(rt + 0x38, event_mem); // ERDP
    wr64(rt + 0x30, table); // ERSTBA, written last: it starts the ring
    let events = EventRing::new(RING_TRBS, event_mem).ok_or("event ring too small")?;

    // Run.
    wr32(op, rd32(op) | 1);
    if !wait_until(100, || rd32(op + 4) & 1 == 0) {
        return Err("controller did not start");
    }
    if rd32(op + 4) & (1 << 2) != 0 {
        return Err("host system error after start");
    }
    Ok(Xhci { vendor: d.header.vendor, device: d.header.device, caps, slots, op, db, rt, dcbaa, handoff, cmd, events, event_mem, keyboard: None })
}

impl Xhci {
    fn port_reg(&self, port: u8) -> u64 {
        self.op + 0x400 + 0x10 * u64::from(port - 1)
    }

    /// The next event the controller has posted, if any; tells the controller how far we have read.
    fn try_event(&mut self) -> Option<Trb> {
        // SAFETY: `event_mem` is the event ring frame from `bring_up`, never freed. The controller writes
        // it by DMA; `EventRing::next` reads it with volatile loads.
        let mem = unsafe { core::slice::from_raw_parts(self.event_mem as *const Trb, RING_TRBS) };
        let event = self.events.next(mem)?;
        // Bit 3 clears the "event handler busy" flag.
        wr64(self.rt + 0x38, self.events.dequeue_pointer() | 1 << 3);
        Some(event)
    }

    /// Waits for a command completion (`kind` 33) or a transfer event (`kind` 32) of `slot` and endpoint
    /// `ep`. Transfer events of the keyboard that arrive meanwhile are handled, others are dropped.
    fn wait_for(&mut self, kind: u8, slot: u8, ep: u8, ms: u64) -> Option<Trb> {
        let t0 = arch::rdtsc();
        let ticks = log::tsc_hz() / 1000 * ms;
        loop {
            while let Some(event) = self.try_event() {
                if event.kind() == kind && (kind != TRB_TRANSFER_EVENT || (event.slot_id() == slot && event.endpoint_id() == ep)) {
                    return Some(event);
                }
                if event.kind() == TRB_TRANSFER_EVENT {
                    self.on_keyboard_event(&event);
                }
            }
            if arch::rdtsc().wrapping_sub(t0) >= ticks {
                return None;
            }
            arch::pause();
        }
    }

    /// Runs one command and returns its completion event.
    fn command(&mut self, trb: Trb) -> Result<Trb, &'static str> {
        self.cmd.enqueue(trb).ok_or("command ring full")?;
        wr32(self.db, 0);
        let event = self.wait_for(TRB_COMMAND_COMPLETION, 0, 0, 1000).ok_or("command timed out")?;
        self.cmd.retire();
        if event.completion_code() != CC_SUCCESS {
            warn!("xhci", "command type {} failed with completion code {}", trb.kind(), event.completion_code());
            return Err("command failed");
        }
        Ok(event)
    }

    /// Resets a connected port and returns its status once the device is enabled.
    fn reset_port(&self, port: u8) -> Option<u32> {
        let at = self.port_reg(port);
        let mut v = rd32(at);
        if v & reg::PORT_POWER == 0 {
            wr32(at, reg::port_neutral(v) | reg::PORT_POWER);
            wait_until(20, || false);
            v = rd32(at);
        }
        if v & reg::PORT_ENABLED == 0 {
            wr32(at, reg::port_reset_command(v));
            if !wait_until(500, || rd32(at) & reg::PORT_RESET_CHANGE != 0) {
                return None;
            }
            wr32(at, reg::port_ack_reset(rd32(at)));
        }
        // Acknowledge the connect change so it does not look new later.
        wr32(at, reg::port_neutral(rd32(at)) | reg::PORT_CONNECT_CHANGE);
        let v = rd32(at);
        (v & reg::PORT_ENABLED != 0 && v & reg::PORT_CONNECTED != 0).then_some(v)
    }

    /// One control transfer on endpoint 0 of `dev`. `data` is the length and direction (true = IN) of the
    /// data stage, which uses the device's descriptor buffer.
    fn control(&mut self, dev: &mut Device, packet: [u8; 8], data: Option<(u16, bool)>) -> Result<(), &'static str> {
        let in_data = data.map(|d| d.1);
        let mut trbs = 0;
        dev.ep0.enqueue(Trb::setup_stage(packet, in_data)).ok_or("control ring full")?;
        trbs += 1;
        if let Some((len, dir_in)) = data {
            dev.ep0.enqueue(Trb::data_stage(dev.buf, len, dir_in)).ok_or("control ring full")?;
            trbs += 1;
        }
        // The status stage runs the other way from the data stage (IN when there is no data stage).
        dev.ep0.enqueue(Trb::status_stage(in_data != Some(true))).ok_or("control ring full")?;
        trbs += 1;
        wr32(self.db + 4 * u64::from(dev.slot), 1);
        let event = self.wait_for(TRB_TRANSFER_EVENT, dev.slot, 1, 1000);
        for _ in 0..trbs {
            dev.ep0.retire();
        }
        match event.map(|e| e.completion_code()) {
            Some(CC_SUCCESS | CC_SHORT_PACKET) => Ok(()),
            Some(code) => {
                warn!("xhci", "control transfer on slot {} failed with completion code {}", dev.slot, code);
                Err("control transfer failed")
            }
            None => Err("control transfer timed out"),
        }
    }

    /// Brings up the device on `port`; when it is a boot keyboard, starts reading its reports.
    fn setup_device(&mut self, port: u8) -> Result<(), &'static str> {
        let portsc = self.reset_port(port).ok_or("port did not enable after reset")?;
        let speed = reg::port_speed(portsc);
        let slot = self.command(Trb::enable_slot())?.slot_id();
        if slot == 0 || slot > self.slots {
            return Err("controller returned a bad slot number");
        }
        let ac64 = self.caps.ac64;
        let csz = self.caps.context_size();
        let out_ctx = dma_page(ac64)?;
        let in_ctx = dma_page(ac64)?;
        let buf = dma_page(ac64)?;
        let (ep0_phys, ep0) = new_ring(ac64)?;
        wr64(self.dcbaa + 8 * u64::from(slot), out_ctx);
        let mut dev = Device { slot, ep0, buf };

        // Address the device with the default control endpoint.
        let assumed = reg::default_ep0_packet(speed);
        // SAFETY: `in_ctx` is a page from `dma_page`; the controller does not write it.
        let ctx = unsafe { page_bytes(in_ctx) };
        let ep0_ctx = Endpoint { dci: xhcictx::DCI_EP0, ep_type: xhcictx::EP_CONTROL, max_packet: assumed, interval: 0, dequeue: ep0_phys, avg_trb_len: 8 };
        if !(xhcictx::input_control(ctx, 0, 0b11) && xhcictx::slot(ctx, csz, speed, port, 1) && xhcictx::endpoint(ctx, csz, &ep0_ctx)) {
            return Err("input context too small");
        }
        self.command(Trb::address_device(in_ctx, slot, false))?;

        // The first 8 bytes of the device descriptor give the real packet size of endpoint 0.
        self.control(&mut dev, [0x80, 6, 0, 1, 0, 0, 8, 0], Some((8, true)))?;
        // SAFETY: `buf` is a page from `dma_page`; the transfer has finished.
        let b = unsafe { page_bytes(buf) };
        let real = if speed >= 4 { 1u16 << b[7].min(15) } else { u16::from(b[7]) };
        if real >= 8 && real != assumed {
            xhcictx::input_control(ctx, 0, 1 << 1);
            xhcictx::set_max_packet(ctx, csz, xhcictx::DCI_EP0, real);
            self.command(Trb::evaluate_context(in_ctx, slot))?;
        }

        // Configuration descriptor: its header tells the full length.
        self.control(&mut dev, [0x80, 6, 0, 2, 0, 0, 9, 0], Some((9, true)))?;
        // SAFETY: as above.
        let b = unsafe { page_bytes(buf) };
        let total = u16::from_le_bytes([b[2], b[3]]).clamp(9, MAX_CONFIG_DESC);
        let [lo, hi] = total.to_le_bytes();
        self.control(&mut dev, [0x80, 6, 0, 2, 0, 0, lo, hi], Some((total, true)))?;
        // SAFETY: as above.
        let b = unsafe { page_bytes(buf) };
        let Some(kb) = usbdesc::find_boot_keyboard(&b[..usize::from(total)]) else {
            info!("xhci", "port {}: device is not a boot keyboard, ignored", port);
            return Ok(());
        };
        if self.keyboard.is_some() {
            info!("xhci", "port {}: a second keyboard is ignored", port);
            return Ok(());
        }

        // Configuration, boot protocol, and no idle repeats from the device.
        self.control(&mut dev, [0x00, 9, kb.config, 0, 0, 0, 0, 0], None)?;
        self.control(&mut dev, [0x21, 0x0B, 0, 0, kb.interface, 0, 0, 0], None)?;
        let _ = self.control(&mut dev, [0x21, 0x0A, 0, 0, kb.interface, 0, 0, 0], None); // stalls on some keyboards: fine

        // Tell the controller about the interrupt IN endpoint.
        let dci = xhcictx::dci_in(kb.endpoint);
        let (ring_phys, ring) = new_ring(ac64)?;
        // SAFETY: `in_ctx` and `out_ctx` are pages from `dma_page`; no command is running.
        let (ctx, out) = unsafe { (page_bytes(in_ctx), page_bytes(out_ctx)) };
        ctx.fill(0);
        let ep = Endpoint {
            dci,
            ep_type: xhcictx::EP_INTERRUPT_IN,
            max_packet: kb.max_packet,
            interval: reg::interrupt_interval_field(speed, kb.interval),
            dequeue: ring_phys,
            avg_trb_len: 8,
        };
        if !(xhcictx::input_control(ctx, 0, 1 | 1 << dci) && xhcictx::copy_slot_from_output(out, ctx, csz) && xhcictx::set_slot_entries(ctx, csz, dci) && xhcictx::endpoint(ctx, csz, &ep)) {
            return Err("input context too small");
        }
        self.command(Trb::configure_endpoint(in_ctx, slot))?;

        let report = dma_page(ac64)?;
        let mut keyboard = Keyboard { port, slot, dci, ring, report, hid: HidKeyboard::new(), queue: KeyQueue::new(), errors: 0 };
        keyboard.ring.enqueue(Trb::normal(report, 8)).ok_or("interrupt ring full")?;
        wr32(self.db + 4 * u64::from(slot), u32::from(dci));
        self.keyboard = Some(keyboard);
        info!("xhci", "port {}: USB keyboard ready (slot {}, speed {})", port, slot, reg::speed_name(speed));
        Ok(())
    }

    /// Looks at every port with something connected until a keyboard is found.
    fn enumerate(&mut self) {
        for port in 1..=self.caps.max_ports {
            if self.keyboard.is_some() {
                break;
            }
            if rd32(self.port_reg(port)) & reg::PORT_CONNECTED == 0 {
                continue;
            }
            if let Err(why) = self.setup_device(port) {
                warn!("xhci", "port {}: device not used: {}", port, why);
            }
        }
    }

    /// A transfer event for the keyboard: read the report, queue the keys, ask for the next report.
    fn on_keyboard_event(&mut self, event: &Trb) {
        let db = self.db;
        let Some(k) = self.keyboard.as_mut() else { return };
        if event.slot_id() != k.slot || event.endpoint_id() != k.dci {
            return;
        }
        k.ring.retire();
        match event.completion_code() {
            CC_SUCCESS | CC_SHORT_PACKET => {
                // SAFETY: `report` is a page from `dma_page`; the transfer that wrote it has completed.
                let report = unsafe { read_volatile(k.report as *const [u8; 8]) };
                let queue = &mut k.queue;
                k.hid.feed(&report, &mut |key| queue.push(key));
            }
            _ => k.errors = k.errors.saturating_add(1),
        }
        if k.errors < 8 && k.ring.enqueue(Trb::normal(k.report, 8)).is_some() {
            wr32(db + 4 * u64::from(k.slot), u32::from(k.dci));
        }
    }
}

/// Finds an xHCI controller (PCI class 0C, subclass 03, interface 30), starts it and sets up a USB
/// keyboard if one is plugged in. Called once at boot, after the PCI scan.
pub fn init() {
    let mut found = None;
    pci::for_each(|d| {
        let h = &d.header;
        if found.is_none() && h.class == 0x0C && h.subclass == 0x03 && h.prog_if == 0x30 {
            found = Some(*d);
        }
    });
    let Some(d) = found else {
        info!("xhci", "no xHCI controller on the PCI bus");
        return;
    };
    match bring_up(&d) {
        Ok(mut x) => {
            info!(
                "xhci",
                "controller {:04x}:{:04x} version {:x}.{:02x}: {} ports, {} slots, {} scratchpad pages, {}",
                x.vendor,
                x.device,
                x.caps.version >> 8,
                x.caps.version & 0xFF,
                x.caps.max_ports,
                x.slots,
                x.caps.scratchpads,
                x.handoff
            );
            for port in 1..=x.caps.max_ports {
                let portsc = rd32(x.port_reg(port));
                if portsc & reg::PORT_CONNECTED != 0 {
                    info!("xhci", "port {}: device connected, speed {}", port, reg::speed_name(reg::port_speed(portsc)));
                }
            }
            x.enumerate();
            // SAFETY: rule A: boot code, before the shell task exists; no interrupt handler touches XHCI.
            *unsafe { XHCI.get() } = Some(x);
        }
        Err(why) => warn!("xhci", "controller at {:02x}:{:02x}.{} not used: {}", d.bus, d.dev, d.func, why),
    }
}

/// The next key from a USB keyboard, if one was pressed. Called from the shell loop; cheap when there is
/// no controller.
pub fn poll_key() -> Option<Key> {
    // SAFETY: rule A: only the shell task and boot code reach XHCI.
    let x = unsafe { XHCI.get() }.as_mut()?;
    while let Some(event) = x.try_event() {
        if event.kind() == TRB_TRANSFER_EVENT {
            x.on_keyboard_event(&event);
        }
    }
    x.keyboard.as_mut()?.queue.pop()
}

/// What the `usb` command shows about the controller.
pub struct Summary {
    pub vendor: u16,
    pub device: u16,
    pub version: u16,
    pub ports: u8,
    pub slots: u8,
    pub scratchpads: u16,
    pub handoff: &'static str,
    pub running: bool,
    /// Port of the USB keyboard in use, if any.
    pub keyboard_port: Option<u8>,
}

pub fn summary() -> Option<Summary> {
    // SAFETY: rule A: only the shell task and boot code reach XHCI.
    let x = unsafe { XHCI.get() }.as_ref()?;
    Some(Summary {
        vendor: x.vendor,
        device: x.device,
        version: x.caps.version,
        ports: x.caps.max_ports,
        slots: x.slots,
        scratchpads: x.caps.scratchpads,
        handoff: x.handoff,
        running: rd32(x.op + 4) & 1 == 0,
        keyboard_port: x.keyboard.as_ref().map(|k| k.port),
    })
}

/// Calls `f(port number, PORTSC)` for every root hub port (numbers start at 1).
pub fn for_each_port(mut f: impl FnMut(u8, u32)) {
    // SAFETY: rule A, as in `summary`.
    let Some(x) = (unsafe { XHCI.get() }).as_ref() else { return };
    for p in 1..=x.caps.max_ports {
        f(p, rd32(x.port_reg(p)));
    }
}
