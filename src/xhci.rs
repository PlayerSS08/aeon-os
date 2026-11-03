//! xHCI USB host controller driver (polling, no interrupts). Part one: find the controller on the PCI bus,
//! take it over from the firmware, reset and start it, and report its ports. Talking to devices comes
//! next. Register decoding is in `xhcireg.rs` and the ring logic in `xhcitrb.rs`, both host-tested.
//!
//! Every wait has a timeout, and any failure leaves the controller unused: the PS/2 keyboard path does
//! not depend on this driver.
//!
//! Global state: `XHCI` follows rule A in `docs/design/globals.md`: only boot code (`init`) and the shell
//! task touch it, never an interrupt handler.

use core::ptr::{read_volatile, write_bytes, write_volatile};

use crate::global::Global;
use crate::xhcireg::{self as reg, Caps};
use crate::xhcitrb::{EventRing, ProducerRing, Trb};
use crate::{arch, log, mm, paging, pci};

/// TRBs per ring: one 4 KiB frame holds 256 of 16 bytes.
const RING_TRBS: usize = 256;
const PAGE_BYTES: usize = 4096;
/// Most device slots this driver asks the controller for.
const MAX_SLOTS: u8 = 32;

// ---- register access ----

fn rd32(addr: u64) -> u32 {
    // SAFETY: `addr` is inside the controller's register window, which `bring_up` mapped uncached, or
    // DMA memory from the frame allocator; both are plain memory-mapped and 4-byte aligned here.
    unsafe { read_volatile(addr as *const u32) }
}

fn wr32(addr: u64, value: u32) {
    // SAFETY: as in `rd32`.
    unsafe { write_volatile(addr as *mut u32, value) }
}

/// 64-bit registers are written as two 32-bit halves, low first, which every controller accepts.
fn wr64(addr: u64, value: u64) {
    wr32(addr, value as u32);
    wr32(addr + 4, (value >> 32) as u32);
}

/// Polls `cond` until it is true or `ms` milliseconds have passed. Returns whether it became true.
fn wait_until(ms: u64, mut cond: impl FnMut() -> bool) -> bool {
    let t0 = arch::rdtsc();
    let ticks = log::tsc_hz() / 1000 * ms;
    loop {
        if cond() {
            return true;
        }
        if arch::rdtsc().wrapping_sub(t0) >= ticks {
            return false;
        }
        arch::pause();
    }
}

/// One zeroed 4 KiB frame the controller may DMA to. Without 64-bit addressing it must be below 4 GiB.
fn dma_page(ac64: bool) -> Result<u64, &'static str> {
    let addr = mm::alloc_frames(1).ok_or("out of memory")?;
    if !ac64 && addr >> 32 != 0 {
        mm::free_frames(addr, 1);
        return Err("the controller needs memory below 4 GiB");
    }
    // SAFETY: a frame just taken from the allocator: page aligned, 4 KiB, identity mapped, used by nobody else.
    unsafe { write_bytes(addr as *mut u8, 0, PAGE_BYTES) };
    Ok(addr)
}

// ---- the controller ----

pub struct Xhci {
    vendor: u16,
    device: u16,
    caps: Caps,
    slots: u8,
    /// Operational registers.
    op: u64,
    handoff: &'static str,
    // Kept for the device part (talking to devices), which reads them.
    #[allow(dead_code)]
    cmd: ProducerRing<'static>,
    #[allow(dead_code)]
    events: EventRing,
    #[allow(dead_code)]
    event_mem: u64,
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
    let cmd_phys = dma_page(caps.ac64)?;
    // SAFETY: a zeroed frame nobody else uses; it is never freed, so the slice is valid for the rest of the run.
    let cmd_mem: &'static mut [Trb] = unsafe { core::slice::from_raw_parts_mut(cmd_phys as *mut Trb, RING_TRBS) };
    let cmd = ProducerRing::new(cmd_mem, cmd_phys).ok_or("command ring too small")?;
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
    Ok(Xhci { vendor: d.header.vendor, device: d.header.device, caps, slots, op, handoff, cmd, events, event_mem })
}

/// Finds an xHCI controller (PCI class 0C, subclass 03, interface 30) and starts it. Called once at boot,
/// after the PCI scan.
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
        Ok(x) => {
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
            // SAFETY: rule A: boot code, before the shell task exists; no interrupt handler touches XHCI.
            *unsafe { XHCI.get() } = Some(x);
            for_each_port(|n, portsc| {
                if portsc & reg::PORT_CONNECTED != 0 {
                    info!("xhci", "port {}: device connected, speed {}", n, reg::speed_name(reg::port_speed(portsc)));
                }
            });
        }
        Err(why) => warn!("xhci", "controller at {:02x}:{:02x}.{} not used: {}", d.bus, d.dev, d.func, why),
    }
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
    })
}

/// Calls `f(port number, PORTSC)` for every root hub port (numbers start at 1).
pub fn for_each_port(mut f: impl FnMut(u8, u32)) {
    // SAFETY: rule A, as in `summary`.
    let Some(x) = (unsafe { XHCI.get() }).as_ref() else { return };
    for p in 1..=x.caps.max_ports {
        f(p, rd32(x.op + 0x400 + 0x10 * u64::from(p - 1)));
    }
}
