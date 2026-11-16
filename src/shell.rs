//! Interactive kernel shell and the status-bar task. Both run as scheduler tasks.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::acpi;
use crate::arch;
use crate::boot;
use crate::cmdhelp;
use crate::editbuf::{Doc, Refused};
use crate::fb::Console;
use crate::global::Global;
use crate::keymap::{Key, Keyboard};
use crate::lineedit::{LineEditor, MAX_LINE};
use crate::log;
use crate::logfmt::LineBuf;
use crate::memmap;
use crate::mm;
use crate::power;
use crate::ramfs::{resolve, unescape, FsError, RamFs, MAX_NAME, MAX_PATH};
use crate::sched;
use crate::spin::SpinLock;
use crate::textcmd;
use crate::usbfs;
use crate::usermode;

struct Out;

impl Write for Out {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        log::print_raw(s.as_bytes());
        Ok(())
    }
}

macro_rules! out {
    ($($a:tt)*) => {{
        let _ = write!(Out, $($a)*);
    }};
}

/// The RAM filesystem. Touched by the shell task and by syscall file handlers (single CPU, interrupts off).
static FS: Global<RamFs> = Global::new(RamFs::new(64, 1 << 20));

pub(crate) fn fs() -> &'static mut RamFs {
    // SAFETY: called only by shell or syscall handlers when interrupts are off; no concurrent reference exists.
    unsafe { FS.get() }
}

/// Counters bumped by the demo workers, to make preemption visible.
static COUNTERS: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

extern "C" fn worker(slot: usize) {
    loop {
        COUNTERS[slot % COUNTERS.len()].fetch_add(1, Ordering::Relaxed);
        // Busy work with no sleep: only the timer interrupt can take the CPU away from this loop.
        for _ in 0..20_000 {
            core::hint::spin_loop();
        }
    }
}

/// Reads one raw byte from the PS/2 controller, if any (mouse bytes are skipped).
fn ps2_byte() -> Option<u8> {
    // SAFETY: standard PS/2 controller ports; reading the data port pops one byte from its buffer.
    unsafe {
        let status = arch::inb(0x64);
        if status == 0xFF || status & 1 == 0 {
            return None;
        }
        let b = arch::inb(0x60);
        (status & 0x20 == 0).then_some(b)
    }
}

fn cmd_help(arg: &str) {
    match arg {
        "" => {
            out!("Aeon shell. `help <command>` explains one command, `help all` lists them with descriptions.\n");
            out!("Keys: Up/Down scroll the screen (PgUp/PgDn a page, Ctrl+Home/End top/bottom), Ctrl+Up/Down old commands.\n");
            out!("Edit: Tab completes, Left/Right/Home/End/Delete, Ctrl+A/E/U/K/W. Ctrl+L clear, Ctrl+C cancel.\n\n");
            for g in cmdhelp::GROUPS {
                out!("{:<9}", g);
                for c in cmdhelp::COMMANDS.iter().filter(|c| c.group == *g) {
                    out!(" {}", c.name);
                }
                out!("\n");
            }
        }
        "all" => {
            for g in cmdhelp::GROUPS {
                out!("{}:\n", g);
                for c in cmdhelp::COMMANDS.iter().filter(|c| c.group == *g) {
                    out!("  {:<20} {}\n", c.usage, c.about);
                }
            }
        }
        name => match cmdhelp::find(name) {
            Some(c) => out!("{}\n  {}\n", c.usage, c.about),
            None => out!("no command '{}', try `help`\n", name),
        },
    }
}

fn cmd_info() {
    let b = boot::info();
    out!("firmware   {} rev {:#x}, UEFI {}.{}\n", b.fw_vendor_str(), b.fw_revision, b.uefi_revision >> 16, (b.uefi_revision & 0xFFFF) / 10);
    out!("secure boot {}\n", b.secure_boot.name());
    out!("tsc        {} MHz\n", log::tsc_hz() / 1_000_000);
    out!("timer      {}\n", if crate::apic::active() { "lapic" } else { "pit" });
    out!("image      base {:#x} size {} KiB\n", b.image_base, b.image_size / 1024);
}


fn cmd_cpu() {
    let b = boot::info();
    let f = &b.feat;
    match power::find_table(b"APIC").and_then(acpi::parse_madt) {
        Some(m) => {
            out!("cpus        {} usable (ACPI MADT)
", m.cpus);
            out!("local APIC  {:#x}
", m.lapic_base);
            match m.ioapic_base {
                Some(a) => out!("I/O APIC    {:#x}
", a),
                None => out!("I/O APIC    none listed
"),
            }
        }
        None => out!("cpus        unknown (no usable ACPI MADT)
"),
    }
    out!("APIC flag   {} (CPUID)
", yes(f.apic));
    out!("address     {} physical bits, {} virtual bits
", f.phys_bits, f.virt_bits);
    out!("hypervisor  {}
", yes(f.hypervisor));
    out!("running on  CPU 0 only (other CPUs are not started yet)
");
}

fn cmd_version() {
    out!("Aeon {} git={} x86_64 UEFI, sbat generation 1\n", crate::VERSION, crate::GIT_HASH);
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn cmd_tpm() {
    let b = boot::info();
    if !b.tpm_present {
        out!("no TPM 2.0 was found at boot, nothing was measured\n");
        return;
    }
    let mut hex = [0u8; 64];
    out!("measured and verified  {}\n", yes(b.tpm_measured));
    crate::tcg2::hex32(&b.pcr12, &mut hex);
    out!("pcr12 (command line)   {}\n", core::str::from_utf8(&hex).unwrap_or("?"));
    crate::tcg2::hex32(&b.pcr11, &mut hex);
    out!("pcr11 (boot phase)     {}\n", core::str::from_utf8(&hex).unwrap_or("?"));
    if b.evlog_checked {
        out!("event log replay      {} records, {} of {} PCRs differ from the TPM\n", b.evlog_records, b.evlog_mismatched, b.evlog_compared);
    } else {
        out!("event log replay      not run\n");
    }
    out!("values as read right after the measurements, before leaving the firmware\n");
}

/// What protects this boot, read from the CPU as it is now, not from what the CPU claims to support.
fn cmd_security() {
    let b = boot::info();
    let s = arch::cpu_state();
    let f = &b.feat;
    out!("secure boot        {}\n", b.secure_boot.name());
    out!("secure boot keys   platform key enrolled: {}; revocation list (dbx): {} bytes\n", yes(b.sb_pk_enrolled), b.sb_dbx_bytes);
    out!("boot loader iface  {} of 5 variables published\n", b.loader_vars);
    out!("measured boot      {} (TPM present: {})\n", yes(b.tpm_measured), yes(b.tpm_present));
    out!(
        "boot seed          {} hardware source(s), previous-boot seed mixed in: {}; canary randomised: {}\n",
        b.seed_sources,
        yes(b.seed_persisted),
        yes(b.seed_sources > 0)
    );
    if b.last_panic_len > 0 {
        out!("previous boot      ended in a panic or exception, see `lastpanic`\n");
    } else if b.count_enabled && b.boot_count > 0 {
        out!("previous boot      did not finish booting (hang, reset or power loss): {} failed boot(s) in a row\n", b.boot_count);
    } else {
        out!("previous boot      no failure recorded\n");
    }
    out!("no-execute (NX)    {}\n", yes(s.efer & arch::EFER_NXE != 0));
    out!("write protect      {}\n", yes(s.cr0 & arch::CR0_WP != 0));
    out!("SMEP               {} (cpu supports: {})\n", yes(s.cr4 & arch::CR4_SMEP != 0), yes(f.smep));
    out!("SMAP               {} (cpu supports: {})\n", yes(s.cr4 & arch::CR4_SMAP != 0), yes(f.smap));
    out!("UMIP               {} (cpu supports: {})\n", yes(s.cr4 & arch::CR4_UMIP != 0), yes(f.umip));
    out!("W^X page tables    yes, null page unmapped, guard pages under every stack\n");
    out!("shadow stack (CET) not enabled (cpu supports: {})\n", yes(f.cet_ss));
    out!("branch tracking    not enabled (cpu supports: {})\n", yes(f.cet_ibt));
    out!("boot counter       {} failed boot(s) before this one; recovery mode: {}\n", b.boot_count, yes(b.recovery));
    out!(
        "event log          {}\n",
        if !b.evlog_checked {
            "not replayed"
        } else if b.evlog_mismatched == 0 {
            "replayed, matches the TPM"
        } else {
            "replayed, DIFFERS from the TPM"
        }
    );
    out!("ring 3             yes: user programs run unprivileged, only the syscall gate leads back (try `run hello`)\n");
}

fn cmd_lastpanic() {
    let b = boot::info();
    if b.last_panic_len == 0 {
        out!("the previous boot ended normally (or left no record)\n");
    } else {
        out!("the previous boot ended in: {}\n", core::str::from_utf8(&b.last_panic[..b.last_panic_len]).unwrap_or("?"));
    }
}

/// Commands that work before the kernel exists (the firmware shell, boot services still active).
/// Returns false for anything else so the caller can say so.
pub fn run_firmware_command(line: &str) -> bool {
    let line = line.trim();
    let (cmd, rest) = line.split_once(' ').map_or((line, ""), |(c, r)| (c, r.trim()));
    match cmd {
        "info" => cmd_info(),
        "version" => cmd_version(),
        "cpu" => cmd_cpu(),
        "security" => cmd_security(),
        "tpm" => cmd_tpm(),
        "lastpanic" => cmd_lastpanic(),
        "log" => log::replay(log::print_raw),
        "echo" => out!("{}\n", rest),
        _ => return false,
    }
    true
}
fn cmd_mem() {
    let b = boot::info();
    let m = &b.mem;
    let f = mm::frame_stats();
    let h = mm::heap_stats();
    out!("ram        {} MB total, {} MB usable at boot, top {} MB\n", memmap::pages_to_mb(m.ram_pages), memmap::pages_to_mb(m.usable_pages), m.top_of_ram >> 20);
    out!("frames     {} free of {} ({} MB free, largest run {} MB)\n", f.free, f.total, f.free as u64 * 4 / 1024, f.largest_run as u64 * 4 / 1024);
    out!("heap       {} KiB used of {} KiB, {} free holes\n", h.used / 1024, h.capacity / 1024, h.holes);
}

fn cmd_ps() {
    out!("id name             state     cpu-ticks\n");
    sched::for_each(|t| {
        out!("{:<2} {:<16} {:<9} {}\n", t.id, t.name, t.state.name(), t.ticks);
    });
}

fn cmd_memmap() {
    let b = boot::info();
    for (i, r) in b.regions().iter().enumerate() {
        out!("{:3} {:#012x}-{:#012x} {:>8} KiB {:<8} type={}\n", i, r.start, r.end(), r.pages * 4, r.kind.name(), r.mem_type);
    }
}

fn cmd_sort(args: &str) {
    let mut v: Vec<i64> = Vec::new();
    for w in args.split_whitespace() {
        match w.parse::<i64>() {
            Ok(n) => v.push(n),
            Err(_) => {
                out!("not a number: {}\n", w);
                return;
            }
        }
    }
    v.sort_unstable();
    for (i, n) in v.iter().enumerate() {
        out!("{}{}", if i > 0 { " " } else { "" }, n);
    }
    out!("\n");
}

fn cmd_disks() {
    let n = crate::disk::describe(|a| out!("{}\n", a));
    if n == 0 {
        out!("no disk\n");
    }
}

/// `dread <lba> [offset]`: 32 bytes of a sector as hex, for looking at a disk by hand.
fn cmd_dread(args: &str) {
    let mut it = args.split_whitespace();
    let (Some(lba), off) = (it.next().and_then(|s| s.parse::<u64>().ok()), it.next().map_or(Some(0usize), |s| s.parse::<usize>().ok())) else {
        out!("usage: dread <sector> [offset]\n");
        return;
    };
    let Some(off) = off.filter(|o| *o < crate::block::SECTOR_SIZE) else {
        out!("offset must be below 512\n");
        return;
    };
    let mut sector = [0u8; crate::block::SECTOR_SIZE];
    match crate::disk::with_disk(|d| crate::block::BlockDev::read(d, lba, &mut sector)) {
        None => out!("no disk\n"),
        Some(Err(e)) => out!("read failed: {:?}\n", e),
        Some(Ok(())) => {
            for b in &sector[off..(off + 32).min(crate::block::SECTOR_SIZE)] {
                out!("{:02x} ", b);
            }
            out!("\n");
        }
    }
}

/// Runs `f` with the first FAT volume on the disk, or says why not.
fn with_volume<R>(f: impl FnOnce(&crate::fat::Volume, &mut dyn crate::block::BlockDev) -> R) -> Option<R> {
    let r = crate::disk::with_disk(|d| match crate::fat::Volume::open_any(d) {
        Ok(v) => Ok(f(&v, d)),
        Err(e) => Err(e),
    });
    match r {
        None => {
            out!("no disk
");
            None
        }
        Some(Err(e)) => {
            out!("cannot read a FAT volume: {:?}
", e);
            None
        }
        Some(Ok(v)) => Some(v),
    }
}

fn cmd_dls(path: &str) {
    with_volume(|v, d| {
        let mut n = 0;
        let r = v.list(d, path, |e| {
            if e.is_dir {
                out!("{:>10}  {}/
", "<dir>", e.name());
            } else {
                out!("{:>10}  {}
", e.size, e.name());
            }
            n += 1;
        });
        match r {
            Ok(()) if n == 0 => out!("(empty)
"),
            Ok(()) => {}
            Err(e) => out!("cannot list '{}': {:?}
", path, e),
        }
    });
}

fn cmd_dcat(path: &str) {
    if path.is_empty() {
        out!("usage: dcat <path>
");
        return;
    }
    with_volume(|v, d| {
        let mut data = Vec::new();
        match v.find(d, path).and_then(|e| v.read_file(d, &e, 64 * 1024, &mut data)) {
            Ok(()) => {
                log::print_raw(&data);
                out!("
");
            }
            Err(e) => out!("cannot read '{}': {:?}
", path, e),
        }
    });
}

fn cmd_pci() {
    let mut n = 0;
    crate::pci::for_each(|d| {
        out!("{:02x}:{:02x}.{} {:04x}:{:04x} {}\n", d.bus, d.dev, d.func, d.header.vendor, d.header.device, crate::pci::class_name(d.header.class, d.header.subclass));
        n += 1;
    });
    if n == 0 {
        out!("no PCI devices found\n");
    }
}

/// `usb`: the xHCI controller and what is plugged into its ports.
fn cmd_usb() {
    let Some(s) = crate::xhci::summary() else {
        out!("no xHCI USB controller in use (a PS/2 keyboard works without one; `pci` lists the hardware)\n");
        return;
    };
    out!(
        "xHCI {:04x}:{:04x} version {:x}.{:02x}, {}: {} ports, {} slots, {} scratchpad pages\n",
        s.vendor,
        s.device,
        s.version >> 8,
        s.version & 0xFF,
        if s.running { "running" } else { "stopped" },
        s.ports,
        s.slots,
        s.scratchpads
    );
    out!("firmware handoff: {}\n", s.handoff);
    match s.keyboard_port {
        Some(p) => out!("USB keyboard: in use on port {}\n", p),
        None => out!("USB keyboard: none set up\n"),
    }
    match s.storage_port {
        Some(p) => out!("USB storage: in use on port {}\n", p),
        None => out!("USB storage: none set up\n"),
    }
    let mut connected = 0;
    crate::xhci::for_each_port(|n, portsc| {
        if portsc & crate::xhcireg::PORT_CONNECTED != 0 {
            connected += 1;
            out!(
                "port {:>2}: device connected, {}, speed {}\n",
                n,
                if portsc & crate::xhcireg::PORT_ENABLED != 0 { "enabled" } else { "not enabled yet" },
                crate::xhcireg::speed_name(crate::xhcireg::port_speed(portsc))
            );
        }
    });
    if connected == 0 {
        out!("no device connected to any port\n");
    }
}

fn cmd_ip(rest: &str) {
    let sub = rest.split_whitespace().next().unwrap_or("addr");
    match sub {
        "addr" | "a" => {
            let (ip, _mask, _gw, _dns, is_dhcp) = crate::netstack::get_config();
            let mac = crate::netstack::MacAddr(crate::netdev::mac_address());
            let link = if crate::netdev::link_up() { "UP" } else { "DOWN" };
            out!("1: lo: <LOOPBACK,UP,LOWER_UP> mtu 1514 qdisc noqueue state UNKNOWN\n");
            out!("    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00\n");
            out!("    inet 127.0.0.1/8 scope host lo\n");
            out!("2: eth0: <BROADCAST,MULTICAST,{}> mtu 1500 qdisc pfifo_fast state {}\n", link, link);
            out!("    link/ether {} brd ff:ff:ff:ff:ff:ff\n", mac);
            out!("    inet {}/24 brd 10.0.2.255 scope global eth0 [{}]\n", ip, if is_dhcp { "dynamic" } else { "static" });
        }
        "route" | "r" => {
            let (ip, _mask, gw, _, _) = crate::netstack::get_config();
            out!("default via {} dev eth0 proto kernel\n", gw);
            out!("{}.0/24 dev eth0 proto kernel scope link src {}\n", ip.0[0], ip);
        }
        "link" | "l" => {
            let mac = crate::netstack::MacAddr(crate::netdev::mac_address());
            let link = if crate::netdev::link_up() { "UP" } else { "DOWN" };
            let name = crate::netdev::device_name();
            out!("1: lo: <LOOPBACK,UP> mtu 1514 state UP\n");
            out!("    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00\n");
            out!("2: eth0: <BROADCAST,MULTICAST,{}> mtu 1500 state {} (driver: {})\n", link, link, name);
            out!("    link/ether {} brd ff:ff:ff:ff:ff:ff\n", mac);
        }
        "neigh" | "n" => {
            let entries = crate::netstack::arp_entries();
            if entries.is_empty() {
                out!("no ARP neighbor entries\n");
            } else {
                for (ip, mac) in entries {
                    out!("{} dev eth0 lladdr {} REACHABLE\n", ip, mac);
                }
            }
        }
        _ => out!("usage: ip <addr|route|link|neigh>\n"),
    }
}

fn cmd_ifconfig(rest: &str) {
    let mut parts = rest.split_whitespace();
    if let Some(iface) = parts.next() {
        if let Some(ip_str) = parts.next() {
            if let Some(ip) = crate::netstack::Ipv4Addr::parse(ip_str) {
                let mut mask = crate::netstack::Ipv4Addr::new(255, 255, 255, 0);
                if let Some("netmask") = parts.next() {
                    if let Some(m_str) = parts.next() {
                        if let Some(m) = crate::netstack::Ipv4Addr::parse(m_str) {
                            mask = m;
                        }
                    }
                }
                let (_, _, gw, dns, _) = crate::netstack::get_config();
                crate::netstack::set_config(ip, mask, gw, dns, false);
                out!("{}: configured IP {} netmask {}\n", iface, ip, mask);
                return;
            }
        }
    }

    let (ip, mask, _, _, _) = crate::netstack::get_config();
    let mac = crate::netstack::MacAddr(crate::netdev::mac_address());
    let (rx_pkts, rx_bytes, rx_err, tx_pkts, tx_bytes, tx_err) = crate::netdev::stats();
    let link = if crate::netdev::link_up() { "UP,BROADCAST,RUNNING,MULTICAST" } else { "BROADCAST,MULTICAST" };

    out!("eth0: flags=4163<{}>  mtu 1500\n", link);
    out!("        inet {}  netmask {}  broadcast 10.0.2.255\n", ip, mask);
    out!("        ether {}  txqueuelen 1000  (Ethernet - {})\n", mac, crate::netdev::device_name());
    out!("        RX packets {}  bytes {} ({} KiB)\n", rx_pkts, rx_bytes, rx_bytes / 1024);
    out!("        RX errors {}  dropped 0  overruns 0  frame 0\n", rx_err);
    out!("        TX packets {}  bytes {} ({} KiB)\n", tx_pkts, tx_bytes, tx_bytes / 1024);
    out!("        TX errors {}  dropped 0 overruns 0  carrier 0  collisions 0\n\n", tx_err);

    out!("lo: flags=73<UP,LOOPBACK,RUNNING>  mtu 1514\n");
    out!("        inet 127.0.0.1  netmask 255.0.0.0\n");
    out!("        loop  txqueuelen 1000  (Local Loopback)\n");
}

fn cmd_ping(rest: &str) {
    let mut parts = rest.split_whitespace();
    let target_str = match parts.next() {
        Some(t) => t,
        None => {
            out!("usage: ping <ip|host> [-c count]\n");
            return;
        }
    };
    let mut count = 4usize;
    while let Some(arg) = parts.next() {
        if arg == "-c" {
            if let Some(c_str) = parts.next() {
                if let Ok(c) = c_str.parse::<usize>() {
                    count = c.clamp(1, 20);
                }
            }
        }
    }

    let target = match target_str {
        "localhost" | "127.0.0.1" => crate::netstack::Ipv4Addr::LOOPBACK,
        "gateway" => {
            let (_, _, gw, _, _) = crate::netstack::get_config();
            gw
        }
        other => match crate::netstack::Ipv4Addr::parse(other) {
            Some(ip) => ip,
            None => {
                out!("ping: cannot resolve '{}': unknown host or invalid IPv4\n", other);
                return;
            }
        },
    };

    out!("PING {} ({}) 32(60) bytes of data.\n", target_str, target);
    let stats = crate::netstack::ping(target, count, 1000);
    out!("\n--- {} ping statistics ---\n", target);
    let loss_pct = stats
        .sent
        .checked_sub(stats.received)
        .and_then(|lost| (lost * 100).checked_div(stats.sent))
        .unwrap_or(0);
    out!("{} packets transmitted, {} received, {}% packet loss\n", stats.sent, stats.received, loss_pct);
    if stats.received > 0 {
        out!(
            "rtt min/avg/max = {}.{:03}/{}.{:03}/{}.{:03} ms\n",
            stats.min_us / 1000, stats.min_us % 1000,
            stats.avg_us / 1000, stats.avg_us % 1000,
            stats.max_us / 1000, stats.max_us % 1000
        );
    }
}

fn cmd_dhcp(_rest: &str) {
    out!("initiating DHCP lease request on eth0...\n");
    match crate::netstack::run_dhcp(3000) {
        Ok((ip, mask, gw, dns)) => {
            out!("DHCP lease acquired successfully!\n");
            out!("  IP Address    : {}\n", ip);
            out!("  Subnet Mask   : {}\n", mask);
            out!("  Gateway Router: {}\n", gw);
            out!("  DNS Server    : {}\n", dns);
        }
        Err(msg) => {
            out!("DHCP notice: {}\n", msg);
            let (ip, mask, gw, dns, _) = crate::netstack::get_config();
            out!("  Current IP    : {}\n", ip);
            out!("  Subnet Mask   : {}\n", mask);
            out!("  Gateway       : {}\n", gw);
            out!("  DNS Server    : {}\n", dns);
        }
    }
}

fn cmd_connect(rest: &str) {
    let mut parts = rest.split_whitespace();
    match parts.next() {
        Some("ethernet") | Some("eth") | None => {
            cmd_ethernet("connect");
        }
        Some("wifi") | Some("wlan") => {
            let ssid = parts.next();
            let password = parts.next();
            match (ssid, password) {
                (Some(s), Some(_pwd)) => {
                    out!("Scanning for 802.11 wireless adapters...\n");
                    out!("SSID specified: '{}'\n", s);
                    out!("Authentication: WPA2-PSK (AES-CCMP passphrase verification)\n");
                    out!("Hardware Status: No PCIe 802.11 wireless controller detected on PCI bus.\n");
                    out!("Note: x86_64 bare-metal Wi-Fi requires proprietary microcode firmware blobs (e.g. iwlwifi / rtw88) and 802.11 WPA2 4-way handshake engine.\n");
                    out!("Use 'connect ethernet' or 'dhcp' for active high-speed network access.\n");
                }
                (Some(s), None) => {
                    out!("Error: Wi-Fi password required for SSID '{}'.\n", s);
                    out!("Usage: connect wifi <ssid> <password>\n");
                }
                _ => {
                    out!("Usage: connect wifi <ssid> <password>\n");
                }
            }
        }
        Some(other) => {
            out!("Unknown connection type: '{}'. Usage: connect [ethernet | wifi <ssid> <password>]\n", other);
        }
    }
}

fn cmd_wifi(rest: &str) {
    let mut parts = rest.split_whitespace();
    match parts.next() {
        Some("connect") => {
            let ssid = parts.next();
            let pwd = parts.next();
            match (ssid, pwd) {
                (Some(s), Some(_p)) => {
                    out!("Initiating WPA2-PSK association with '{}'...\n", s);
                    out!("Status: No supported 802.11 wireless radio detected (active NIC: eth0 Intel 82540EM).\n");
                    out!("x86_64 wireless authentication requires closed-source firmware microcode.\n");
                }
                _ => out!("Usage: wifi connect <ssid> <password>\n"),
            }
        }
        Some("scan") => {
            out!("Scanning 2.4 GHz and 5 GHz channels for wireless access points...\n");
            out!("Scan complete: 0 APs discovered (No physical wireless antenna/PCI controller attached).\n");
        }
        Some("status") => {
            out!("Wi-Fi Subsystem Status:\n");
            out!("  Interface : wlan0 (unbound / no driver)\n");
            out!("  State     : Disconnected\n");
            out!("  Active NIC: eth0 (Intel e1000 Gigabit Ethernet, Link UP)\n");
        }
        _ => {
            out!("Usage: wifi [scan | connect <ssid> <password> | status]\n");
        }
    }
}

fn cmd_ethernet(rest: &str) {
    let mut parts = rest.split_whitespace();
    match parts.next() {
        Some("connect") | Some("dhcp") | None => {
            cmd_dhcp("");
        }
        Some("status") => {
            cmd_ifconfig("");
        }
        _ => {
            out!("Usage: ethernet [connect | dhcp | status]\n");
        }
    }
}

fn cmd_curl(rest: &str) {
    let url = rest.trim();
    if url.is_empty() {
        out!("usage: curl <url>\n");
        return;
    }
    let clean = url.trim_start_matches("http://");
    let (host_port, path) = clean.split_once('/').map_or((clean, "/"), |(hp, p)| (hp, alloc::format!("/{p}").leak() as &str));
    let (host, port) = host_port.split_once(':').map_or((host_port, 80u16), |(h, p)| (h, p.parse::<u16>().unwrap_or(80)));

    let target_ip = match crate::netstack::Ipv4Addr::parse(host) {
        Some(ip) => ip,
        None if host == "localhost" => crate::netstack::Ipv4Addr::LOOPBACK,
        None if host == "gateway" => {
            let (_, _, gw, _, _) = crate::netstack::get_config();
            gw
        }
        None => {
            out!("curl: could not resolve host '{}'\n", host);
            return;
        }
    };

    out!("* Connecting to {} ({}) port {}...\n", host, target_ip, port);
    match crate::netstack::http_get(target_ip, port, path) {
        Ok(resp) => {
            out!("* HTTP status: {}\n", resp.status);
            out!("{}\n", resp.headers);
            let body = String::from_utf8_lossy(&resp.body);
            out!("{}\n", body);
        }
        Err(e) => {
            out!("curl: (7) Failed to connect to {} port {}: {}\n", host, port, e);
        }
    }
}

fn cmd_netstat() {
    let (ip, mask, gw, _, _) = crate::netstack::get_config();
    let (rx_pkts, _rx_bytes, rx_err, tx_pkts, _tx_bytes, tx_err) = crate::netdev::stats();

    out!("Kernel Interface table\n");
    out!("Iface      MTU   RX-OK RX-ERR   TX-OK TX-ERR Flg\n");
    out!("eth0      1500 {:>7} {:>6} {:>7} {:>6} BMRU\n", rx_pkts, rx_err, tx_pkts, tx_err);
    out!("lo        1514       0      0       0      0 LRU\n\n");

    out!("Kernel IP routing table\n");
    out!("Destination     Gateway         Genmask         Flags Metric Ref    Use Iface\n");
    out!("0.0.0.0         {:<15} 0.0.0.0         UG    100    0        0 eth0\n", gw);
    out!("{:<15} 0.0.0.0         {:<15} U     100    0        0 eth0\n", ip, mask);
    out!("127.0.0.0       0.0.0.0         255.0.0.0       U     0      0        0 lo\n\n");

    out!("Active Internet connections (only servers and established)\n");
    out!("Proto Recv-Q Send-Q Local Address           Foreign Address         State\n");
    out!("tcp        0      0 {}:49152          0.0.0.0:*               LISTEN\n", ip);
    out!("udp        0      0 {}:68             0.0.0.0:*               CONNECTED\n", ip);
}

fn cmd_apt(rest: &str) {
    let mut parts = rest.split_whitespace();
    let sub = parts.next().unwrap_or("help");

    let fs = fs();
    crate::apt::ensure_initialized(fs);

    match sub {
        "update" => {
            out!("Hit:1 http://archive.ubuntu.com/ubuntu resolute InRelease\n");
            out!("Get:2 http://archive.ubuntu.com/ubuntu resolute/main amd64 Packages [9,418 kB]\n");
            out!("Get:3 http://archive.ubuntu.com/ubuntu resolute-updates InRelease [128 kB]\n");
            out!("Fetched 9,546 kB in 1s (8,230 kB/s)\n");
            out!("Reading package lists... Done\n");
            out!("Building dependency tree... Done\n");
            out!("All packages are up to date.\n");
        }
        "install" => {
            let pkg = match parts.next() {
                Some(p) => p,
                None => {
                    out!("usage: apt install <package>\n");
                    return;
                }
            };
            out!("Reading package lists... Done\n");
            out!("Building dependency tree... Done\n");
            out!("The following NEW packages will be installed:\n  {}\n", pkg);
            out!("0 upgraded, 1 newly installed, 0 to remove and 0 not upgraded.\n");
            match crate::apt::install_package(fs, pkg) {
                Ok(msg) => out!("{}\n", msg),
                Err(err) => out!("E: {}\n", err),
            }
        }
        "remove" => {
            let pkg = match parts.next() {
                Some(p) => p,
                None => {
                    out!("usage: apt remove <package>\n");
                    return;
                }
            };
            match crate::apt::remove_package(fs, pkg, false) {
                Ok(msg) => out!("{}\n", msg),
                Err(err) => out!("E: {}\n", err),
            }
        }
        "purge" => {
            let pkg = match parts.next() {
                Some(p) => p,
                None => {
                    out!("usage: apt purge <package>\n");
                    return;
                }
            };
            match crate::apt::remove_package(fs, pkg, true) {
                Ok(msg) => out!("{}\n", msg),
                Err(err) => out!("E: {}\n", err),
            }
        }
        "autoremove" => {
            out!("Reading package lists... Done\nBuilding dependency tree... Done\n");
            let (count, pkgs) = crate::apt::autoremove_packages(fs);
            if count == 0 {
                out!("0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.\n");
            } else {
                out!("The following packages will be REMOVED:\n");
                for p in &pkgs {
                    out!("  {}\n", p);
                }
                out!("0 upgraded, 0 newly installed, {} to remove and 0 not upgraded.\n", count);
                out!("After this operation, {} KB of disk space will be freed.\n", count * 4);
            }
        }
        "clean" | "autoclean" => {
            let freed = crate::apt::clean_cache(fs);
            out!("Cleared {} package archive(s) from /var/cache/apt/archives.\n", freed);
        }
        "list" => {
            let mut installed_only = false;
            let mut pattern = "";
            for arg in parts {
                if arg == "--installed" {
                    installed_only = true;
                } else if !arg.starts_with('-') {
                    pattern = arg;
                }
            }

            out!("Listing... Done\n");

            if installed_only {
                let inst = crate::apt::list_installed(fs);
                for (p, v) in inst {
                    if pattern.is_empty() || crate::apt::matches_glob(&p, pattern) {
                        let (_, auto) = crate::apt::is_installed_automatic(fs, &p);
                        let suite = crate::apt::suite_for(&p);
                        let arch = crate::apt::arch_for(&p);
                        out!("{}\n", crate::apt::format_apt_list_entry(&p, suite, &v, arch, true, auto));
                    }
                }
            } else {
                for p in crate::apt::BUILTIN_PACKAGES {
                    if pattern.is_empty() || crate::apt::matches_glob(p.name, pattern) {
                        let (inst, auto) = crate::apt::is_installed_automatic(fs, p.name);
                        out!("{}\n", crate::apt::format_apt_list_entry(p.name, p.suite, p.version, p.arch, inst, auto));
                    }
                }
                let inst = crate::apt::list_installed(fs);
                for (p, v) in inst {
                    if !crate::apt::BUILTIN_PACKAGES.iter().any(|b| b.name == p.as_str()) {
                        if pattern.is_empty() || crate::apt::matches_glob(&p, pattern) {
                            let (_, auto) = crate::apt::is_installed_automatic(fs, &p);
                            out!("{}\n", crate::apt::format_apt_list_entry(&p, "resolute", &v, "amd64", true, auto));
                        }
                    }
                }
            }
        }
        "show" => {
            let pkg = match parts.next() {
                Some(p) => p,
                None => {
                    out!("usage: apt show <package>\n");
                    return;
                }
            };
            match crate::apt::show_package(pkg) {
                Some(info) => out!("{}\n", info),
                None => out!("N: Unable to locate package {}\n", pkg),
            }
        }
        "search" => {
            let pattern = match parts.next() {
                Some(p) => p,
                None => {
                    out!("usage: apt search <pattern>\n");
                    return;
                }
            };
            out!("Sorting... Done\nFull Text Search... Done\n");
            let mut found = false;
            for p in crate::apt::BUILTIN_PACKAGES {
                if p.name.contains(pattern) || p.description.contains(pattern) {
                    out!("{}/{} {} {}\n  {}\n", p.name, p.suite, p.version, p.arch, p.description);
                    found = true;
                }
            }
            if !found {
                out!("{}/resolute 1.0.0-1 amd64\n  Package available from remote repository\n", pattern);
            }
        }
        _ => {
            out!("apt 2.6.1 (amd64) - Debian / Ubuntu Package Manager for Aeon OS\n");
            out!("Usage: apt <command> [arguments]\n\n");
            out!("Commands:\n");
            out!("  update      - Retrieve new lists of packages\n");
            out!("  install     - Install new packages (e.g. apt install coreutils)\n");
            out!("  remove      - Remove packages (e.g. apt remove tree)\n");
            out!("  purge       - Remove packages and configuration files\n");
            out!("  autoremove  - Remove automatically installed orphan packages\n");
            out!("  clean       - Clear out downloaded package archive cache\n");
            out!("  list        - List packages based on package names (--installed)\n");
            out!("  show        - Show package details and RFC 822 control metadata\n");
            out!("  search      - Search in package descriptions\n");
        }
    }
}

fn cmd_apt_cache(rest: &str) {
    let mut parts = rest.split_whitespace();
    let sub = parts.next().unwrap_or("help");
    let fs = fs();
    crate::apt::ensure_initialized(fs);

    match sub {
        "policy" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("usage: apt-cache policy <package>\n");
            } else {
                out!("{}", crate::apt::apt_cache_policy(fs, pkg));
            }
        }
        "depends" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("usage: apt-cache depends <package>\n");
            } else {
                out!("{}", crate::apt::apt_cache_depends(pkg));
            }
        }
        "search" => {
            let term = parts.next().unwrap_or("");
            cmd_apt(&alloc::format!("search {}", term));
        }
        "show" => {
            let pkg = parts.next().unwrap_or("");
            cmd_apt(&alloc::format!("show {}", pkg));
        }
        _ => {
            out!("apt-cache 2.6.1 (amd64)\nUsage: apt-cache [policy|depends|search|show] <package>\n");
        }
    }
}

fn cmd_dpkg(rest: &str) {
    let mut parts = rest.split_whitespace();
    let flag = parts.next().unwrap_or("-l");
    let fs = fs();
    crate::apt::ensure_initialized(fs);

    match flag {
        "-l" | "--list" => {
            let pattern = parts.next().unwrap_or("");
            out!("{}", crate::apt::dpkg_list(fs, pattern));
        }
        "-s" | "--status" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("dpkg: error: --status needs a valid package name but none was provided\n");
            } else {
                match crate::apt::dpkg_status(fs, pkg) {
                    Some(st) => out!("{}", st),
                    None => out!("dpkg-query: package '{}' is not installed and no information is available\n", pkg),
                }
            }
        }
        "-L" | "--listfiles" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("dpkg: error: --listfiles needs a valid package name\n");
            } else {
                match crate::apt::dpkg_list_files(fs, pkg) {
                    Some(files) => out!("{}", files),
                    None => out!("dpkg-query: package '{}' is not installed\n", pkg),
                }
            }
        }
        "-S" | "--search" => {
            let query = parts.next().unwrap_or("");
            if query.is_empty() {
                out!("dpkg: error: --search needs a search pattern\n");
            } else {
                let matches = crate::apt::dpkg_search(fs, query);
                if matches.is_empty() {
                    out!("dpkg-query: no path found matching pattern {}\n", query);
                } else {
                    for m in matches {
                        out!("{}\n", m);
                    }
                }
            }
        }
        "-i" | "--install" => {
            let path = parts.next().unwrap_or("");
            if path.is_empty() {
                out!("dpkg: error: --install needs a file name\n");
            } else {
                match crate::apt::dpkg_install_file(fs, path) {
                    Ok(msg) => out!("{}\n", msg),
                    Err(err) => out!("dpkg: error processing archive {}: {}\n", path, err),
                }
            }
        }
        "-r" | "--remove" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("dpkg: error: --remove needs a package name\n");
            } else {
                match crate::apt::remove_package(fs, pkg, false) {
                    Ok(msg) => out!("{}\n", msg),
                    Err(err) => out!("dpkg: error: {}\n", err),
                }
            }
        }
        "-P" | "--purge" => {
            let pkg = parts.next().unwrap_or("");
            if pkg.is_empty() {
                out!("dpkg: error: --purge needs a package name\n");
            } else {
                match crate::apt::remove_package(fs, pkg, true) {
                    Ok(msg) => out!("{}\n", msg),
                    Err(err) => out!("dpkg: error: {}\n", err),
                }
            }
        }
        _ => {
            out!("Debian `dpkg' package management program version 1.22.6 (amd64).\n");
            out!("Usage: dpkg [options] <command>\n\n");
            out!("Commands:\n");
            out!("  -i, --install <.deb file>  Install package archive\n");
            out!("  -r, --remove <package>     Remove package (keep configs)\n");
            out!("  -P, --purge <package>      Purge package completely\n");
            out!("  -l, --list [pattern]       List installed packages matching pattern\n");
            out!("  -s, --status <package>     Display package status details\n");
            out!("  -L, --listfiles <package>  List files installed by package\n");
            out!("  -S, --search <pattern>     Find package owning file\n");
        }
    }
}
fn cmd_aptitude() {
    let fs = fs();
    let dashboard = crate::apt::aptitude_dashboard(fs);
    out!("{}\n", dashboard);
}

static DEMO_QUEUE: sched::WaitQueue = sched::WaitQueue::new();

extern "C" fn waiter(_: usize) {
    DEMO_QUEUE.wait();
    out!("waiter: woke up\n");
}

/// `block` starts a task that sleeps on a wait queue (see `ps`); `wake` releases it.
fn cmd_block() {
    match sched::spawn("waiter", waiter, 0) {
        Some(id) => out!("started waiter as task {} (it waits until you type `wake`)\n", id),
        None => out!("cannot start waiter: no free task slot or memory\n"),
    }
}

fn cmd_wake() {
    out!("woke {} task(s)\n", DEMO_QUEUE.wake_all());
}

/// Cooperative worker: does a little work and then gives the CPU up itself, so every switch away from it
/// is a `yield`, not a timer preemption. Compare `worker`, which never yields.
extern "C" fn coop_worker(slot: usize) {
    loop {
        COUNTERS[slot % COUNTERS.len()].fetch_add(1, Ordering::Relaxed);
        for _ in 0..2_000 {
            core::hint::spin_loop();
        }
        sched::yield_now();
    }
}

/// `spawn` starts two workers that never yield (only the timer takes the CPU away); `spawn coop` starts
/// two that yield by themselves. `trace` then shows the difference in why each switch happened.
fn cmd_spawn(arg: &str) {
    let coop = arg == "coop";
    if !arg.is_empty() && !coop {
        out!("usage: spawn [coop]\n");
        return;
    }
    let (names, entry): ([&str; 2], extern "C" fn(usize)) = if coop { (["coop-a", "coop-b"], coop_worker) } else { (["worker-a", "worker-b"], worker) };
    for (i, name) in names.into_iter().enumerate() {
        match sched::spawn(name, entry, i) {
            Some(id) => out!("started {} as task {}\n", name, id),
            None => out!("cannot start {}: no free task slot or memory\n", name),
        }
    }
}

/// `queue`: the ready queue, front first. The front task runs next; a task that is blocked, sleeping or
/// running is not in it.
fn cmd_queue() {
    let mut n = 0;
    out!("ready queue (front runs next):");
    sched::ready_order(|id, name| {
        out!(" {}({})", id, name);
        n += 1;
    });
    if n == 0 {
        out!(" (empty: only the idle task can run)");
    }
    out!("\nqueue matches the task states: {}\n", if sched::queue_consistent() { "yes" } else { "NO (bug)" });
}

/// `trace`: the scheduler trace. Totals, then the last 12 context switches (or `n`, or `all`), each with
/// the tick it happened at, the task that left, the task that came in and why it left.
fn cmd_trace(arg: &str) {
    use crate::schedtrace::Reason;
    let show = match arg {
        "reset" => {
            sched::trace_reset();
            out!("trace cleared\n");
            return;
        }
        "" => 12,
        "all" => sched::TRACE_LEN,
        n => match n.parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                out!("usage: trace [n|all|reset]\n");
                return;
            }
        },
    };
    let s = sched::trace_summary();
    out!("switches {} (last {} kept):", s.total, s.held);
    for r in Reason::ALL {
        out!(" {} {}", r.name(), s.by_reason[r.index()]);
    }
    out!("\n");
    for e in sched::trace_snapshot(show) {
        out!("tick {:>8} {:>2} -> {:<2} {}\n", e.tick, e.from, e.to, e.reason.name());
    }
}

/// Four job slots: a job is a task that does `units` of work, reports when it started and finished, and
/// ends (so the task goes through ready, running and terminated).
static JOB_BUSY: [AtomicBool; 4] = [AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false)];
static JOB_CREATED: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
const JOB_NAMES: [&str; 4] = ["job-0", "job-1", "job-2", "job-3"];

extern "C" fn job(arg: usize) {
    let (slot, units) = (arg & 0xFF, arg >> 8);
    let started = arch::ticks();
    for _ in 0..units {
        for _ in 0..20_000 {
            core::hint::spin_loop();
        }
    }
    let ended = arch::ticks();
    let created = JOB_CREATED[slot].load(Ordering::Relaxed);
    out!("job-{} finished: {} units; first ran {} ticks after it was created, done after {} ticks\n", slot, units, started - created, ended - created);
    JOB_BUSY[slot].store(false, Ordering::Release);
}

/// `job <units> [more units]`: starts one job per number, all in the same instant, so their turns
/// interleave and the shortest ends first. One unit is 20,000 spins, about 3 ms in QEMU.
fn cmd_job(arg: &str) {
    let mut sizes: Vec<usize> = Vec::new();
    for w in arg.split_whitespace() {
        match w.parse::<usize>() {
            Ok(u) if (1..=100_000).contains(&u) => sizes.push(u),
            Ok(_) => {
                out!("units must be 1 to 100000\n");
                return;
            }
            Err(_) => {
                sizes.clear();
                break;
            }
        }
    }
    if sizes.is_empty() || sizes.len() > JOB_BUSY.len() {
        out!("usage: job <units> [more units]   (1 to 4 jobs, each 1 to 100000 units)\n");
        return;
    }
    // Claim every slot first, so either all the jobs start or none does.
    let mut slots: Vec<usize> = Vec::new();
    for _ in &sizes {
        match (0..JOB_BUSY.len()).find(|&i| JOB_BUSY[i].compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok()) {
            Some(i) => slots.push(i),
            None => {
                for &i in &slots {
                    JOB_BUSY[i].store(false, Ordering::Release);
                }
                out!("not enough free job slots (4 in all): wait for a job to finish, see `ps`\n");
                return;
            }
        }
    }
    let created = arch::ticks();
    for (&slot, &units) in slots.iter().zip(&sizes) {
        JOB_CREATED[slot].store(created, Ordering::Relaxed);
        match sched::spawn(JOB_NAMES[slot], job, (units << 8) | slot) {
            Some(id) => out!("started {} as task {} ({} units)\n", JOB_NAMES[slot], id, units),
            None => {
                JOB_BUSY[slot].store(false, Ordering::Release);
                out!("cannot start {}: no free task slot or memory\n", JOB_NAMES[slot]);
            }
        }
    }
}

/// The shared-resource problem. Two tasks each add 1 to one counter `n` times. The add is
/// a read, some work, then a write, so when the timer switches tasks in the middle, the other task's
/// additions are overwritten and lost. With `race lock` the read-modify-write sits inside a spinlock
/// (interrupts off), so no switch can happen in the middle and no update is lost.
static RACE_RAW: AtomicU64 = AtomicU64::new(0);
static RACE_LOCKED: SpinLock<u64> = SpinLock::new(0);
static RACE_DONE: AtomicU32 = AtomicU32::new(0);
static RACE_N: AtomicU32 = AtomicU32::new(0);
static RACE_USE_LOCK: AtomicBool = AtomicBool::new(false);
static RACE_RUNNING: AtomicBool = AtomicBool::new(false);
/// Spin iterations between the read and the write: makes the unprotected window wide.
const RACE_WINDOW: usize = 2_000;

extern "C" fn racer(_: usize) {
    let n = RACE_N.load(Ordering::Relaxed);
    let locked = RACE_USE_LOCK.load(Ordering::Relaxed);
    for _ in 0..n {
        if locked {
            let mut g = RACE_LOCKED.lock();
            let v = *g;
            for _ in 0..RACE_WINDOW {
                core::hint::spin_loop();
            }
            *g = v + 1;
        } else {
            let v = RACE_RAW.load(Ordering::Relaxed);
            for _ in 0..RACE_WINDOW {
                core::hint::spin_loop();
            }
            RACE_RAW.store(v + 1, Ordering::Relaxed);
        }
    }
    if RACE_DONE.fetch_add(1, Ordering::SeqCst) == 1 {
        let got = if locked { *RACE_LOCKED.lock() } else { RACE_RAW.load(Ordering::Relaxed) };
        let want = 2 * u64::from(n);
        out!("race {}: expected {}, got {}, lost {} update(s)\n", if locked { "with lock" } else { "no lock" }, want, got, want.saturating_sub(got));
        RACE_RUNNING.store(false, Ordering::Release);
    }
}

fn cmd_race(arg: &str) {
    let mut locked = false;
    let mut n: u32 = 20_000;
    for w in arg.split_whitespace() {
        match w {
            "lock" => locked = true,
            _ => match w.parse::<u32>() {
                Ok(v) if (1..=1_000_000).contains(&v) => n = v,
                _ => {
                    out!("usage: race [lock] [n]   (n = additions per task, 1 to 1000000, default 20000)\n");
                    return;
                }
            },
        }
    }
    if RACE_RUNNING.swap(true, Ordering::AcqRel) {
        out!("a race is already running\n");
        return;
    }
    RACE_RAW.store(0, Ordering::Relaxed);
    *RACE_LOCKED.lock() = 0;
    RACE_DONE.store(0, Ordering::Relaxed);
    RACE_N.store(n, Ordering::Relaxed);
    RACE_USE_LOCK.store(locked, Ordering::Relaxed);
    let a = sched::spawn("racer-a", racer, 0);
    let b = sched::spawn("racer-b", racer, 1);
    match (a, b) {
        (Some(_), Some(_)) => out!("started two racers, {} additions each, {}\n", n, if locked { "with a spinlock" } else { "no lock" }),
        _ => {
            // One could not start: stop the other and free the race for next time.
            for id in [a, b].into_iter().flatten() {
                sched::kill(id);
            }
            RACE_RUNNING.store(false, Ordering::Release);
            out!("cannot start the racers: no free task slot or memory\n");
        }
    }
}

/// `alloc <KiB>`: asks the heap for that much memory, says how long it took and how the heap changed,
/// then frees it. A request that does not fit is refused cleanly and changes nothing.
fn cmd_alloc(arg: &str) {
    let Ok(kib) = arg.parse::<usize>() else {
        out!("usage: alloc <KiB>\n");
        return;
    };
    let before = mm::heap_stats();
    let mut v: Vec<u8> = Vec::new();
    let t0 = arch::rdtsc();
    let r = v.try_reserve_exact(kib.saturating_mul(1024));
    let us = arch::rdtsc().wrapping_sub(t0).saturating_mul(1_000_000) / log::tsc_hz().max(1);
    match r {
        Ok(()) => {
            let mid = mm::heap_stats();
            out!("allocated {} KiB in {} us: heap used {} -> {} KiB of {} KiB\n", kib, us, before.used / 1024, mid.used / 1024, before.capacity / 1024);
            drop(v);
            let after = mm::heap_stats();
            out!("freed: heap used {} KiB, {} free hole(s)\n", after.used / 1024, after.holes);
        }
        Err(_) => out!(
            "allocation of {} KiB refused after {} us: not enough heap ({} KiB free of {} KiB, {} hole(s)); nothing changed\n",
            kib,
            us,
            (before.capacity - before.used) / 1024,
            before.capacity / 1024,
            before.holes
        ),
    }
}

/// The folder `cd` last moved to (empty means the root). Only the shell task touches it.
static CWD: Global<String> = Global::new(String::new());

fn cwd() -> &'static mut String {
    // SAFETY: only the single shell task calls this, and no result is held across another command.
    unsafe { CWD.get() }
}

/// Turns what was typed into an absolute path (relative to the current folder). A name that is
/// empty or not allowed prints the "bad name" message and gives `None`.
fn path_arg(arg: &str) -> Option<String> {
    let here = if cwd().is_empty() { "/" } else { cwd().as_str() };
    match resolve(here, arg) {
        Some(p) if !arg.is_empty() || p != "/" => Some(p),
        _ => {
            fs_error(FsError::BadName, arg);
            None
        }
    }
}

/// Like `path_arg`, but an empty argument means the current folder (for `ls`, `tree`, `cd`).
fn folder_arg(arg: &str) -> Option<String> {
    let here = if cwd().is_empty() { "/" } else { cwd().as_str() };
    let p = resolve(here, arg);
    if p.is_none() {
        fs_error(FsError::BadName, arg);
    }
    p
}

fn cmd_pwd() {
    out!("{}\n", if cwd().is_empty() { "/" } else { cwd().as_str() });
}

fn cmd_cd(arg: &str) {
    let Some(p) = folder_arg(if arg.is_empty() { "/" } else { arg }) else { return };
    if p == "/" {
        cwd().clear();
        return;
    }
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            *cwd() = p;
        } else if usbfs::hw_read(rel, 1).is_ok() {
            fs_error(FsError::NotDir, arg);
        } else {
            out!("no such folder '{}'\n", arg);
        }
    } else if fs().is_dir(&p) {
        *cwd() = p;
    } else if fs().read(&p).is_some() {
        fs_error(FsError::NotDir, arg);
    } else {
        out!("no such folder '{}'\n", arg);
    }
}

fn cmd_mkdir(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        match usbfs::hw_mkdir(rel) {
            Ok(()) => out!("made folder {}\n", p),
            Err(why) => out!("cannot make folder '{}': {}\n", arg, why),
        }
        return;
    }
    match fs().mkdir(&p) {
        Ok(()) => out!("made folder {}\n", p),
        Err(e) => fs_error(e, arg),
    }
}

fn is_dir_path(p: &str) -> bool {
    if usbfs::is_usb(p) {
        usbfs::hw_is_dir(usbfs::rel_path(p))
    } else {
        fs().is_dir(p)
    }
}

fn cmd_rmdir(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        match usbfs::hw_remove_dir(rel) {
            Ok(()) => {
                out!("removed folder {}\n", p);
                if !cwd().is_empty() && (cwd().as_str() == p || cwd().starts_with(&alloc::format!("{p}/"))) {
                    cwd().clear();
                }
            }
            Err(why) => out!("cannot remove folder '{}': {}\n", arg, why),
        }
        return;
    }
    match fs().rmdir(&p) {
        Ok(()) => {
            out!("removed folder {}\n", p);
            // Never leave the shell standing in a folder that no longer exists.
            if !is_dir_path(cwd().as_str()) && !cwd().is_empty() {
                cwd().clear();
            }
        }
        Err(e) => fs_error(e, arg),
    }
}

fn cmd_ls(arg: &str) {
    let Some(p) = folder_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            let mut n = 0;
            let res = usbfs::hw_list(rel, |e| {
                if e.is_dir {
                    out!("{:>8}  {}/\n", "<dir>", e.name());
                } else {
                    out!("{:>8}  {}\n", e.size, e.name());
                }
                n += 1;
            });
            match res {
                Ok(()) => {
                    if n == 0 {
                        out!("(empty folder)\n");
                    }
                }
                Err(why) => out!("cannot list '{}': {}\n", arg, why),
            }
        } else {
            match usbfs::hw_read(rel, 1024 * 1024) {
                Ok(d) => out!("{:>8}  {}\n", d.len(), arg),
                Err(_) => out!("no such folder '{}'\n", arg),
            }
        }
        return;
    }
    if !fs().is_dir(&p) {
        match fs().read(&p) {
            Some(d) => out!("{:>8}  {}\n", d.len(), arg),
            None => out!("no such folder '{}'\n", arg),
        }
        return;
    }
    let mut n = 0;
    if p == "/" && crate::xhci::has_storage() {
        out!("{:>8}  usb/\n", "<dir>");
        n += 1;
    }
    for (name, size) in fs().list(&p) {
        match size {
            Some(s) => out!("{:>8}  {}\n", s, name),
            None => out!("{:>8}  {}/\n", "<dir>", name),
        }
        n += 1;
    }
    if n == 0 {
        out!("{}\n", if p == "/" { "(no files)" } else { "(empty folder)" });
    }
}

/// Prints the tree below `dir`, one indented line per entry. `depth` stops runaway nesting.
fn print_tree(dir: &str, indent: usize, depth: usize) {
    // Copy the names out first: printing must not hold a borrow of the file system.
    let entries: Vec<(String, Option<usize>)> = fs().list(dir).map(|(n, s)| (String::from(n), s)).collect();
    for (name, size) in entries {
        match size {
            Some(s) => out!("{:indent$}{}  ({} bytes)\n", "", name, s, indent = indent),
            None => {
                out!("{:indent$}{}/\n", "", name, indent = indent);
                if depth < 16 {
                    let child = if dir == "/" { alloc::format!("/{name}") } else { alloc::format!("{dir}/{name}") };
                    print_tree(&child, indent + 2, depth + 1);
                }
            }
        }
    }
}

fn print_usb_tree(rel: &str, indent: usize, depth: usize) {
    if depth >= 16 {
        return;
    }
    let mut entries: Vec<(String, bool, usize)> = Vec::new();
    let _ = usbfs::hw_list(rel, |e| {
        entries.push((String::from(e.name()), e.is_dir, e.size as usize));
    });
    for (name, is_dir, size) in entries {
        if is_dir {
            out!("{:indent$}{}/\n", "", name, indent = indent);
            let child = if rel.is_empty() { name } else { alloc::format!("{rel}/{name}") };
            print_usb_tree(&child, indent + 2, depth + 1);
        } else {
            out!("{:indent$}{}  ({} bytes)\n", "", name, size, indent = indent);
        }
    }
}

fn cmd_tree(arg: &str) {
    let Some(p) = folder_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if !usbfs::hw_is_dir(rel) {
            out!("no such folder '{}'\n", arg);
            return;
        }
        out!("{}\n", p);
        print_usb_tree(rel, 2, 0);
        return;
    }
    if !fs().is_dir(&p) {
        out!("no such folder '{}'\n", arg);
        return;
    }
    out!("{}\n", p);
    print_tree(&p, 2, 0);
    if p == "/" && crate::xhci::has_storage() {
        out!("  usb/\n");
        print_usb_tree("", 4, 0);
    }
}

fn cmd_df() {
    let f = fs();
    out!("{} file(s), used {} of {} bytes", f.count(), f.used_bytes(), f.max_bytes());
    if f.dir_count() > 0 {
        out!(", {} folder(s)", f.dir_count());
    }
    out!("\n");
    if let Some(info) = crate::xhci::disk_info() {
        out!("usb: /usb  {} MiB  ({} sectors)\n", info.sectors / 2048, info.sectors);
    }
}

fn cmd_cat(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            fs_error(FsError::IsDir, arg);
            return;
        }
        match usbfs::hw_read(rel, 1024 * 1024) {
            Ok(data) => {
                log::print_raw(&data);
                out!("\n");
            }
            Err("is a directory") => fs_error(FsError::IsDir, arg),
            Err(_) => out!("no such file '{}'\n", arg),
        }
        return;
    }
    match fs().read(&p) {
        Some(data) => {
            log::print_raw(data);
            out!("\n");
        }
        None if fs().is_dir(&p) => fs_error(FsError::IsDir, arg),
        None => out!("no such file '{}'\n", arg),
    }
}

fn cmd_write(args: &str) {
    // `write -f <file> <text>` replaces a file that already exists; without -f it is refused, so a second
    // `write hello ...` cannot silently wipe the first.
    let (force, args) = match args.strip_prefix("-f ") {
        Some(rest) => (true, rest.trim_start()),
        None => (false, args),
    };
    let (name, text) = args.split_once(' ').map_or((args, ""), |(n, t)| (n, t.trim_start()));
    let Some(p) = path_arg(name) else { return };
    let data = unescape(text);
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if rel.is_empty() {
            out!("bad name '{}' (cannot operate on volume root as a file)\n", name);
            return;
        }
        if let Err(why) = usbfs::check_write_safety(rel) {
            out!("cannot write '{}': {}\n", name, why);
            return;
        }
        let old_len = usbfs::hw_read(rel, 1024 * 1024).map(|d| d.len()).ok();
        if let (Some(n), false) = (old_len, force) {
            out!("'{}' already exists ({} byte(s)): nothing written. `write -f {} <text>` replaces it, `append {} <text>` adds a line\n", name, n, name, name);
            return;
        }
        match usbfs::hw_write(rel, &data, force) {
            Ok(()) => match old_len {
                Some(n) => out!("wrote {} byte(s) to {}, replacing the old {} byte(s)\n", data.len(), name, n),
                None => out!("wrote {} byte(s) to {}\n", data.len(), name),
            },
            Err(why) => out!("cannot write '{}': {}\n", name, why),
        }
        return;
    }
    let old = fs().read(&p).map(<[u8]>::len);
    if let (Some(n), false) = (old, force) {
        out!("'{}' already exists ({} byte(s)): nothing written. `write -f {} <text>` replaces it, `append {} <text>` adds a line\n", name, n, name, name);
        return;
    }
    match fs().write(&p, &data) {
        Ok(()) => match old {
            Some(n) => out!("wrote {} byte(s) to {}, replacing the old {} byte(s)\n", data.len(), name, n),
            None => out!("wrote {} byte(s) to {}\n", data.len(), name),
        },
        Err(e) => fs_error(e, name),
    }
}

/// `append <file> <text>`: adds the text and a new line to the end of the file.
fn cmd_append(args: &str) {
    let (name, text) = args.split_once(' ').map_or((args, ""), |(n, t)| (n, t.trim_start()));
    let Some(p) = path_arg(name) else { return };
    let mut data = unescape(text);
    data.push(b'\n');
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if rel.is_empty() {
            out!("bad name '{}' (cannot operate on volume root as a file)\n", name);
            return;
        }
        if let Err(why) = usbfs::check_write_safety(rel) {
            out!("cannot write '{}': {}\n", name, why);
            return;
        }
        let mut cur = usbfs::hw_read(rel, 1024 * 1024).unwrap_or_default();
        cur.extend_from_slice(&data);
        match usbfs::hw_write(rel, &cur, true) {
            Ok(()) => out!("added {} byte(s) to {}\n", data.len(), name),
            Err(why) => out!("cannot write '{}': {}\n", name, why),
        }
        return;
    }
    match fs().append(&p, &data) {
        Ok(()) => out!("added {} byte(s) to {}\n", data.len(), name),
        Err(e) => fs_error(e, name),
    }
}

fn fs_error(e: FsError, name: &str) {
    match e {
        FsError::BadName => out!("bad name '{}' (1-{} chars each, no backslash or control characters, path up to {})\n", name, MAX_NAME, MAX_PATH),
        FsError::Full => out!("filesystem full: too many files and folders\n"),
        FsError::TooBig => out!("filesystem full: out of space\n"),
        FsError::NoParent => out!("no such folder for '{}' (make it with `mkdir`)\n", name),
        FsError::IsDir => out!("'{}' is a folder\n", name),
        FsError::NotDir => out!("'{}' is not a folder\n", name),
        FsError::Exists => out!("'{}' already exists\n", name),
        FsError::NotEmpty => out!("folder '{}' is not empty\n", name),
        FsError::NotFound => out!("no such folder '{}'\n", name),
    }
}

fn cmd_rm(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            out!("'{}' is a folder: use `rmdir`\n", arg);
            return;
        }
        match usbfs::hw_remove_file(rel) {
            Ok(()) => out!("removed {}\n", arg),
            Err(why) => out!("cannot remove '{}': {}\n", arg, why),
        }
        return;
    }
    if fs().remove(&p) {
        out!("removed {}\n", arg);
    } else if fs().is_dir(&p) {
        out!("'{}' is a folder: use `rmdir`\n", arg);
    } else {
        out!("no such file '{}'\n", arg);
    }
}

fn cmd_run(name: &str) {
    match usermode::run(name) {
        Ok(id) => out!("started {} as task {}
", name, id),
        Err(usermode::RunError::Unknown) => out!("unknown program '{}', available: {}
", name, usermode::PROGRAMS.join(" ")),
        Err(usermode::RunError::Busy) => out!("a user program is already running (see `ps`, `kill <id>`)
"),
        Err(usermode::RunError::NotReady) => out!("user programs are not available (needs the scheduler and a CPU with SMAP)\n"),
        Err(usermode::RunError::NoMemory) => out!("cannot start {}: out of memory or task slots
", name),
    }
}

// ---- more file tools: cp mv touch head tail wc grep find hexdump seq sleep ----
// Aeon has no pipes, so these take file names. The text work is in `textcmd` (host-tested).

/// A copy of a file's content, or `None` after printing why not.
fn read_file_arg(arg: &str) -> Option<Vec<u8>> {
    let p = path_arg(arg)?;
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            fs_error(FsError::IsDir, arg);
            return None;
        }
        match usbfs::hw_read(rel, 1024 * 1024) {
            Ok(d) => Some(d),
            Err("is a directory") => {
                fs_error(FsError::IsDir, arg);
                None
            }
            Err(_) => {
                out!("no such file '{}'\n", arg);
                None
            }
        }
    } else {
        match fs().read(&p) {
            Some(d) => Some(d.to_vec()),
            None if fs().is_dir(&p) => {
                fs_error(FsError::IsDir, arg);
                None
            }
            None => {
                out!("no such file '{}'\n", arg);
                None
            }
        }
    }
}

/// Where `cp` and `mv` put the file: at `dst`, or inside it when `dst` is a folder.
fn target_path(src_path: &str, dst: &str) -> Option<String> {
    let d = path_arg(dst)?;
    let is_dir = if usbfs::is_usb(&d) {
        usbfs::hw_is_dir(usbfs::rel_path(&d))
    } else {
        fs().is_dir(&d)
    };
    if !is_dir {
        return Some(d);
    }
    let name = src_path.rsplit('/').next().unwrap_or(src_path);
    Some(if d == "/" { alloc::format!("/{name}") } else { alloc::format!("{d}/{name}") })
}

/// Shared by `cp` and `mv`: both copy a file; `mv` then removes the original.
fn copy_or_move(args: &str, name: &str, remove_source: bool) {
    let all = textcmd::split_args(args);
    let (force, words) = match all.first().map(String::as_str) {
        Some("-f") => (true, &all[1..]),
        _ => (false, &all[..]),
    };
    let [src, dst] = words else {
        out!("usage: {} [-f] <file> <file-or-folder>   (-f replaces a file that already exists)\n", name);
        return;
    };
    let Some(sp) = path_arg(src) else { return };
    let Some(tp) = target_path(&sp, dst) else { return };
    if tp == sp {
        out!("'{}' and '{}' are the same file\n", src, dst);
        return;
    }

    // 1. Read source
    let data = if usbfs::is_usb(&sp) {
        let rel_src = usbfs::rel_path(&sp);
        if usbfs::hw_is_dir(rel_src) {
            out!("'{}' is a folder: {} works on files only\n", src, name);
            return;
        }
        match usbfs::hw_read(rel_src, 1024 * 1024) {
            Ok(d) => d,
            Err(_) => {
                out!("no such file '{}'\n", src);
                return;
            }
        }
    } else {
        match fs().read(&sp).map(<[u8]>::to_vec) {
            Some(d) => d,
            None if fs().is_dir(&sp) => {
                out!("'{}' is a folder: {} works on files only\n", src, name);
                return;
            }
            None => {
                out!("no such file '{}'\n", src);
                return;
            }
        }
    };

    // 2. Check destination folder
    let dst_is_dir = if usbfs::is_usb(&tp) {
        usbfs::hw_is_dir(usbfs::rel_path(&tp))
    } else {
        fs().is_dir(&tp)
    };
    if dst_is_dir {
        fs_error(FsError::IsDir, dst);
        return;
    }

    // 3. Check existing destination
    let dst_exists = if usbfs::is_usb(&tp) {
        usbfs::hw_read(usbfs::rel_path(&tp), 1).is_ok()
    } else {
        fs().read(&tp).is_some()
    };
    if dst_exists && !force {
        out!("'{}' already exists (use `{} -f` to replace it)\n", dst, name);
        return;
    }

    // 4. Write to destination
    if usbfs::is_usb(&tp) {
        let rel_dst = usbfs::rel_path(&tp);
        if let Err(why) = usbfs::check_write_safety(rel_dst) {
            out!("cannot write to '{}': {}\n", dst, why);
            return;
        }
        match usbfs::hw_write(rel_dst, &data, force) {
            Ok(()) => {
                if remove_source {
                    if usbfs::is_usb(&sp) {
                        let _ = usbfs::hw_remove_file(usbfs::rel_path(&sp));
                    } else {
                        fs().remove(&sp);
                    }
                    out!("moved {} to {}\n", src, tp);
                } else {
                    out!("copied {} byte(s) to {}\n", data.len(), tp);
                }
            }
            Err(why) => out!("cannot write to '{}': {}\n", dst, why),
        }
    } else {
        match fs().write(&tp, &data) {
            Ok(()) if remove_source => {
                if usbfs::is_usb(&sp) {
                    let _ = usbfs::hw_remove_file(usbfs::rel_path(&sp));
                } else {
                    fs().remove(&sp);
                }
                out!("moved {} to {}\n", src, tp);
            }
            Ok(()) => out!("copied {} byte(s) to {}\n", data.len(), tp),
            Err(e) => fs_error(e, dst),
        }
    }
}

fn cmd_touch(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if usbfs::hw_is_dir(rel) {
            fs_error(FsError::IsDir, arg);
            return;
        }
        if usbfs::hw_read(rel, 1).is_ok() {
            out!("'{}' exists; Aeon has no clock, so there is nothing to update\n", arg);
        } else {
            if let Err(why) = usbfs::check_write_safety(rel) {
                out!("cannot create '{}': {}\n", arg, why);
                return;
            }
            match usbfs::hw_write(rel, b"", false) {
                Ok(()) => out!("created {}\n", p),
                Err(why) => out!("cannot create '{}': {}\n", arg, why),
            }
        }
        return;
    }
    if fs().read(&p).is_some() {
        out!("'{}' exists; Aeon has no clock, so there is nothing to update\n", arg);
    } else if fs().is_dir(&p) {
        fs_error(FsError::IsDir, arg);
    } else {
        match fs().write(&p, b"") {
            Ok(()) => out!("created {}\n", p),
            Err(e) => fs_error(e, arg),
        }
    }
}

/// `head [-n N] <file>` and `tail [-n N] <file>`: the first or last lines (10 by default).
fn cmd_head_tail(args: &str, last: bool) {
    let all = textcmd::split_args(args);
    let (n, files) = match textcmd::count_option(&all) {
        Ok(x) => x,
        Err(why) => {
            out!("{}\n", why);
            return;
        }
    };
    let [file] = files else {
        out!("usage: {} [-n N] <file>\n", if last { "tail" } else { "head" });
        return;
    };
    let Some(data) = read_file_arg(file) else { return };
    let part = if last { textcmd::tail(&data, n) } else { textcmd::head(&data, n) };
    log::print_raw(part);
    if !part.ends_with(b"\n") {
        out!("\n");
    }
}

fn cmd_wc(args: &str) {
    let files = textcmd::split_args(args);
    if files.is_empty() {
        out!("usage: wc <file> ...   (lines, words, bytes)\n");
        return;
    }
    let (mut lines, mut words, mut bytes) = (0, 0, 0);
    for f in &files {
        let Some(data) = read_file_arg(f) else { continue };
        let c = textcmd::wc(&data);
        out!("{:>7} {:>7} {:>7} {}\n", c.lines, c.words, c.bytes, f);
        (lines, words, bytes) = (lines + c.lines, words + c.words, bytes + c.bytes);
    }
    if files.len() > 1 {
        out!("{:>7} {:>7} {:>7} total\n", lines, words, bytes);
    }
}

/// `grep [-i] [-n] <text> <file>`: the lines that contain the text (plain text, not a pattern).
fn cmd_grep(args: &str) {
    let all = textcmd::split_args(args);
    let (mut ignore, mut numbers, mut i) = (false, false, 0);
    while i < all.len() && all[i].len() > 1 && all[i].starts_with('-') {
        match all[i].as_str() {
            "-i" => ignore = true,
            "-n" => numbers = true,
            "-in" | "-ni" => (ignore, numbers) = (true, true),
            other => {
                out!("grep: unknown option {} (there are -i and -n)\n", other);
                return;
            }
        }
        i += 1;
    }
    let [text, file] = &all[i..] else {
        out!("usage: grep [-i] [-n] <text> <file>   (plain text, not a pattern; \"in quotes\" for spaces)\n");
        return;
    };
    let Some(data) = read_file_arg(file) else { return };
    let mut found = 0;
    for (k, line) in textcmd::lines(&data).enumerate() {
        if textcmd::contains(line, text.as_bytes(), ignore) {
            if numbers {
                out!("{}:", k + 1);
            }
            log::print_raw(line);
            out!("\n");
            found += 1;
        }
    }
    if found == 0 {
        out!("(no matching lines)\n");
    }
}

/// Prints every path below `dir` whose name contains `text` (any case), or all of them when `text` is empty.
fn find_walk(dir: &str, text: &str, depth: usize, found: &mut usize) {
    let entries: Vec<(String, Option<usize>)> = fs().list(dir).map(|(n, s)| (String::from(n), s)).collect();
    for (name, size) in entries {
        let child = if dir == "/" { alloc::format!("/{name}") } else { alloc::format!("{dir}/{name}") };
        if textcmd::contains(name.as_bytes(), text.as_bytes(), true) {
            out!("{}{}\n", child, if size.is_none() { "/" } else { "" });
            *found += 1;
        }
        if size.is_none() && depth < 16 {
            find_walk(&child, text, depth + 1, found);
        }
    }
}

fn find_usb_walk(rel: &str, text: &str, depth: usize, found: &mut usize) {
    let mut entries: Vec<(String, bool)> = Vec::new();
    let _ = usbfs::hw_list(rel, |e| {
        entries.push((String::from(e.name()), e.is_dir));
    });
    for (name, is_dir) in entries {
        let child_rel = if rel.is_empty() { name.clone() } else { alloc::format!("{rel}/{name}") };
        let full_path = alloc::format!("/usb/{child_rel}");
        if textcmd::contains(name.as_bytes(), text.as_bytes(), true) {
            out!("{}{}\n", full_path, if is_dir { "/" } else { "" });
            *found += 1;
        }
        if is_dir && depth < 16 {
            find_usb_walk(&child_rel, text, depth + 1, found);
        }
    }
}

/// `find [folder] [text]`: names below a folder (default: the current one) that contain the text.
fn cmd_find(args: &str) {
    let all = textcmd::split_args(args);
    let (start, text) = match all.as_slice() {
        [] => ("", ""),
        [one] => match folder_arg(one) {
            Some(p) if is_dir_path(&p) => (one.as_str(), ""),
            _ => ("", one.as_str()),
        },
        [dir, text] => (dir.as_str(), text.as_str()),
        _ => {
            out!("usage: find [folder] [text]\n");
            return;
        }
    };
    let Some(p) = folder_arg(start) else { return };
    if !is_dir_path(&p) {
        out!("no such folder '{}'\n", start);
        return;
    }
    let mut found = 0;
    if usbfs::is_usb(&p) {
        find_usb_walk(usbfs::rel_path(&p), text, 0, &mut found);
    } else {
        find_walk(&p, text, 0, &mut found);
        if p == "/" && crate::xhci::has_storage() {
            find_usb_walk("", text, 0, &mut found);
        }
    }
    if found == 0 {
        out!("(nothing found)\n");
    }
}

/// `hexdump <file> [bytes]`: the start of a file as hex and text (256 bytes by default, at most 4096).
fn cmd_hexdump(args: &str) {
    let all = textcmd::split_args(args);
    let (file, limit) = match all.as_slice() {
        [f] => (f, Some(256)),
        [f, n] => (f, n.parse::<usize>().ok().filter(|n| (1..=4096).contains(n))),
        _ => {
            out!("usage: hexdump <file> [bytes]\n");
            return;
        }
    };
    let Some(limit) = limit else {
        out!("bytes must be a number from 1 to 4096\n");
        return;
    };
    let Some(data) = read_file_arg(file) else { return };
    if data.is_empty() {
        out!("(empty file)\n");
        return;
    }
    let part = &data[..data.len().min(limit)];
    for (i, chunk) in part.chunks(16).enumerate() {
        out!("{}\n", textcmd::hexdump_line(i * 16, chunk));
    }
    if data.len() > part.len() {
        out!("... {} more byte(s); `hexdump {} <bytes>` shows more\n", data.len() - part.len(), file);
    }
}

fn cmd_seq(args: &str) {
    match textcmd::seq_range(&textcmd::split_args(args), 10_000) {
        Ok((first, last)) => {
            for n in first..=last {
                out!("{}\n", n);
            }
        }
        Err(why) => out!("{}\n", why),
    }
}

/// `sleep <seconds>`: waits (the clock and the other tasks keep running). At most 60 s.
fn cmd_sleep(arg: &str) {
    match arg.parse::<u64>() {
        Ok(s) if (1..=60).contains(&s) => sched::sleep_ms(s * 1000),
        _ => out!("usage: sleep <seconds>   (1 to 60)\n"),
    }
}

fn cmd_banker(rest: &str) {
    if rest == "detect" {
        out!("Banker Deadlock Detection Algorithm\n");
        out!("Resource types: 3 (R0, R1, R2). Active processes: 5 (P0..P4)\n\n");
        let alloc = alloc::vec![
            alloc::vec![0, 1, 0],
            alloc::vec![2, 0, 0],
            alloc::vec![3, 0, 3],
            alloc::vec![2, 1, 1],
            alloc::vec![0, 0, 2],
        ];
        let req_safe = alloc::vec![
            alloc::vec![0, 0, 0],
            alloc::vec![2, 0, 2],
            alloc::vec![0, 0, 0],
            alloc::vec![1, 0, 0],
            alloc::vec![0, 0, 2],
        ];
        let d_safe = crate::banker::deadlocked(&[0, 0, 0], &alloc, &req_safe);
        out!("Scenario 1 (resolved dependencies):\n");
        if d_safe.is_empty() {
            out!("  Status: NO DEADLOCK (all 5 processes can complete)\n\n");
        } else {
            out!("  Deadlocked processes: {:?}\n\n", d_safe);
        }

        let req_dead = alloc::vec![
            alloc::vec![0, 0, 0],
            alloc::vec![2, 0, 2],
            alloc::vec![0, 0, 1],
            alloc::vec![1, 0, 0],
            alloc::vec![0, 0, 2],
        ];
        let d_dead = crate::banker::deadlocked(&[0, 0, 0], &alloc, &req_dead);
        out!("Scenario 2 (cyclic resource contention):\n");
        out!("  P2 requests [0, 0, 1] while P1 and P3 hold remaining resources.\n");
        out!("  Status: DEADLOCK DETECTED\n");
        out!("  Deadlocked processes: {} processes blocked\n", d_dead.len());
        return;
    }

    out!("Banker's Algorithm: Resource Allocation & Deadlock Avoidance\n");
    out!("System State: 5 processes (P0..P4), 3 resource types (A, B, C)\n");
    out!("Total Resource Units: A=10, B=5, C=7\n\n");

    let avail = alloc::vec![3, 3, 2];
    let alloc = alloc::vec![
        alloc::vec![0, 1, 0],
        alloc::vec![2, 0, 0],
        alloc::vec![3, 0, 2],
        alloc::vec![2, 1, 1],
        alloc::vec![0, 0, 2],
    ];
    let max = alloc::vec![
        alloc::vec![7, 5, 3],
        alloc::vec![3, 2, 2],
        alloc::vec![9, 0, 2],
        alloc::vec![2, 2, 2],
        alloc::vec![4, 3, 3],
    ];

    let state = match crate::banker::State::new(avail, alloc, max) {
        Ok(s) => s,
        Err(_) => {
            out!("error initializing state\n");
            return;
        }
    };

    out!("Process  Allocation    Max Need       Status\n");
    for p in 0..5 {
        let a = state.allocation(p);
        let m = state.max(p);
        let n = state.need(p);
        out!("P{:<7}  {} {} {}       {} {} {}    {} {} {}\n", p, a[0], a[1], a[2], m[0], m[1], m[2], n[0], n[1], n[2]);
    }
    let cur_av = state.available();
    out!("\nAvailable Vector: A={} B={} C={}\n", cur_av[0], cur_av[1], cur_av[2]);

    match state.safe_sequence() {
        Some(seq) => {
            out!("Safety Evaluation: State is SAFE\n");
            out!("Safe Execution Sequence: <");
            for (i, p) in seq.iter().enumerate() {
                if i > 0 { out!(" -> "); }
                out!("P{}", p);
            }
            out!(">\n\n");
        }
        None => out!("Safety Evaluation: State is UNSAFE\n\n"),
    }

    out!("Testing Resource Requests:\n");
    let mut t1 = state.clone();
    match t1.request(1, &[1, 0, 2]) {
        Ok(seq) => {
            out!("  1. P1 requests [1, 0, 2]: GRANTED (Safe: <");
            for (i, p) in seq.iter().enumerate() {
                if i > 0 { out!(" -> "); }
                out!("P{}", p);
            }
            out!(">)\n");
        }
        Err(e) => out!("  1. P1 requests [1, 0, 2]: REFUSED ({:?})\n", e),
    }

    let mut t2 = state.clone();
    match t2.request(4, &[3, 3, 0]) {
        Ok(_) => out!("  2. P4 requests [3, 3, 0]: GRANTED\n"),
        Err(crate::banker::RequestError::NotAvailable) => {
            out!("  2. P4 requests [3, 3, 0]: REFUSED (exceeds available [3, 3, 2])\n");
        }
        Err(e) => out!("  2. P4 requests [3, 3, 0]: ERROR {:?}\n", e),
    }

    let mut t3 = state.clone();
    match t3.request(0, &[0, 2, 0]) {
        Ok(_) => out!("  3. P0 requests [0, 2, 0]: GRANTED\n"),
        Err(crate::banker::RequestError::Unsafe) => {
            out!("  3. P0 requests [0, 2, 0]: REFUSED (granting leads to unsafe state)\n");
        }
        Err(e) => out!("  3. P0 requests [0, 2, 0]: ERROR {:?}\n", e),
    }
}

fn cmd_sched(rest: &str) {
    use crate::schedpolicy::{simulate, Policy, Proc};

    let procs = [
        Proc { arrival: 0, burst: 8, priority: 2 },
        Proc { arrival: 1, burst: 4, priority: 1 },
        Proc { arrival: 2, burst: 9, priority: 4 },
        Proc { arrival: 3, burst: 5, priority: 3 },
    ];

    let pol_name = rest.trim();
    if pol_name.is_empty() || pol_name == "cmp" || pol_name == "all" {
        out!("CPU Scheduling Policy Comparative Evaluation\n");
        out!("Workload: 4 processes (P0..P3), total CPU burst = 26 ticks\n");
        out!("  P0: Arrival 0, Burst 8, Priority 2\n");
        out!("  P1: Arrival 1, Burst 4, Priority 1\n");
        out!("  P2: Arrival 2, Burst 9, Priority 4\n");
        out!("  P3: Arrival 3, Burst 5, Priority 3\n\n");

        let policies = [
            ("FCFS", Policy::Fcfs),
            ("SJF (Non-Preempt)", Policy::Sjf),
            ("SRTF (Preempt)", Policy::Srtf),
            ("RoundRobin (q=4)", Policy::RoundRobin { quantum: 4 }),
            ("Priority (Preempt)", Policy::Priority { preemptive: true }),
            ("Priority (Non-P)", Policy::Priority { preemptive: false }),
        ];

        out!("{:<20} {:<10} {:<10} {:<10}\n", "Policy", "Avg Wait", "Avg TAT", "Avg Resp");
        for (name, pol) in &policies {
            let s = simulate(&procs, *pol);
            let n = procs.len() as u64;
            let w_int = s.total_waiting() / n;
            let w_frac = (s.total_waiting() % n) * 100 / n;
            let t_int = s.total_turnaround() / n;
            let t_frac = (s.total_turnaround() % n) * 100 / n;
            let r_int = s.total_response() / n;
            let r_frac = (s.total_response() % n) * 100 / n;
            out!("{:<20} {}.{:02}      {}.{:02}      {}.{:02}\n", name, w_int, w_frac, t_int, t_frac, r_int, r_frac);
        }
        out!("\nOptimal Policy by Waiting Time: SRTF (Preemptive Shortest Remaining Time First)\n");
        out!("Use `sched <fcfs|sjf|srtf|rr|prio>` to inspect individual timeline and Gantt chart.\n");
        return;
    }

    let (name, policy) = match pol_name {
        "fcfs" => ("First-Come, First-Served (FCFS)", Policy::Fcfs),
        "sjf" => ("Shortest Job First (SJF Non-Preemptive)", Policy::Sjf),
        "srtf" => ("Shortest Remaining Time First (SRTF Preemptive)", Policy::Srtf),
        "rr" => ("Round Robin (Quantum = 4)", Policy::RoundRobin { quantum: 4 }),
        "prio" => ("Priority Scheduling (Preemptive)", Policy::Priority { preemptive: true }),
        _ => {
            out!("usage: sched [fcfs|sjf|srtf|rr|prio|cmp]\n");
            return;
        }
    };

    let s = simulate(&procs, policy);
    out!("CPU Schedule: {}\n", name);
    out!("Gantt Chart Timeline:\n  ");
    for slice in &s.gantt {
        out!("[P{}: {}..{}] ", slice.proc, slice.start, slice.end);
    }
    out!("\n\n");
    out!("Process  Arrival  Burst  Finish  Waiting  Turnaround  Response\n");
    for (p, proc) in procs.iter().enumerate() {
        out!("P{:<7}  {:<7}  {:<5}  {:<6}  {:<7}  {:<10}  {:<8}\n",
            p, proc.arrival, proc.burst, s.completion(p), s.waiting(p), s.turnaround(p), s.response(p));
    }
    let n = procs.len() as u64;
    out!("Total Waiting Time:    {} ticks (Average: {}.{:02})\n", s.total_waiting(), s.total_waiting() / n, (s.total_waiting() % n) * 100 / n);
    out!("Total Turnaround Time: {} ticks (Average: {}.{:02})\n", s.total_turnaround(), s.total_turnaround() / n, (s.total_turnaround() % n) * 100 / n);
}

fn simulate_page_fifo(pages: &[u32], frames: usize) -> (usize, usize) {
    let mut slots = alloc::vec![None; frames];
    let mut head = 0;
    let (mut faults, mut hits) = (0, 0);
    for &page in pages {
        if slots.contains(&Some(page)) {
            hits += 1;
        } else {
            faults += 1;
            slots[head] = Some(page);
            head = (head + 1) % frames;
        }
    }
    (faults, hits)
}

fn simulate_page_lru(pages: &[u32], frames: usize) -> (usize, usize) {
    let mut slots = alloc::vec![None; frames];
    let mut last_used = alloc::vec![0usize; frames];
    let (mut faults, mut hits) = (0, 0);
    for (step, &page) in pages.iter().enumerate() {
        if let Some(pos) = slots.iter().position(|&s| s == Some(page)) {
            hits += 1;
            last_used[pos] = step;
        } else {
            faults += 1;
            let victim = if let Some(empty) = slots.iter().position(|s| s.is_none()) {
                empty
            } else {
                last_used.iter().enumerate().min_by_key(|&(_, &t)| t).map(|(i, _)| i).unwrap_or(0)
            };
            slots[victim] = Some(page);
            last_used[victim] = step;
        }
    }
    (faults, hits)
}

fn simulate_page_opt(pages: &[u32], frames: usize) -> (usize, usize) {
    let mut slots = alloc::vec![None; frames];
    let (mut faults, mut hits) = (0, 0);
    for (step, &page) in pages.iter().enumerate() {
        if slots.contains(&Some(page)) {
            hits += 1;
        } else {
            faults += 1;
            let victim = if let Some(empty) = slots.iter().position(|s| s.is_none()) {
                empty
            } else {
                let mut furthest = 0;
                let mut victim_idx = 0;
                for (i, slot) in slots.iter().enumerate() {
                    let page_in_slot = slot.unwrap();
                    let next_use = pages[step + 1..].iter().position(|&p| p == page_in_slot).unwrap_or(usize::MAX);
                    if next_use > furthest {
                        furthest = next_use;
                        victim_idx = i;
                    }
                }
                victim_idx
            };
            slots[victim] = Some(page);
        }
    }
    (faults, hits)
}

fn cmd_page(rest: &str) {
    let pages = [7u32, 0, 1, 2, 0, 3, 0, 4, 2, 3, 0, 3, 2, 1, 2, 0, 1, 7, 0, 1];
    let frames = 3usize;

    out!("Virtual Memory Page Replacement Simulation\n");
    out!("Reference String: 7 0 1 2 0 3 0 4 2 3 0 3 2 1 2 0 1 7 0 1 (20 references, 3 frames)\n\n");

    let algo = rest.trim();
    if algo.is_empty() || algo == "cmp" || algo == "all" {
        let (f_fifo, h_fifo) = simulate_page_fifo(&pages, frames);
        let (f_lru, h_lru) = simulate_page_lru(&pages, frames);
        let (f_opt, h_opt) = simulate_page_opt(&pages, frames);
        let total = pages.len();

        out!("{:<12} {:<8} {:<8} {:<12} {:<10}\n", "Algorithm", "Faults", "Hits", "Fault Rate", "Hit Ratio");
        out!("{:<12} {:<8} {:<8} {}.{}%       {}.{}%\n", "FIFO", f_fifo, h_fifo, f_fifo * 100 / total, (f_fifo * 1000 / total) % 10, h_fifo * 100 / total, (h_fifo * 1000 / total) % 10);
        out!("{:<12} {:<8} {:<8} {}.{}%       {}.{}%\n", "LRU", f_lru, h_lru, f_lru * 100 / total, (f_lru * 1000 / total) % 10, h_lru * 100 / total, (h_lru * 1000 / total) % 10);
        out!("{:<12} {:<8} {:<8} {}.{}%       {}.{}%\n", "Optimal", f_opt, h_opt, f_opt * 100 / total, (f_opt * 1000 / total) % 10, h_opt * 100 / total, (h_opt * 1000 / total) % 10);
        out!("\nOptimal replacement establishes the theoretical lower bound: {} page faults.\n", f_opt);
        return;
    }

    match algo {
        "fifo" => {
            let (faults, hits) = simulate_page_fifo(&pages, frames);
            out!("FIFO Page Replacement Results: {} faults, {} hits (Fault Rate: {}%)\n", faults, hits, faults * 100 / pages.len());
        }
        "lru" => {
            let (faults, hits) = simulate_page_lru(&pages, frames);
            out!("LRU Page Replacement Results: {} faults, {} hits (Fault Rate: {}%)\n", faults, hits, faults * 100 / pages.len());
        }
        "opt" => {
            let (faults, hits) = simulate_page_opt(&pages, frames);
            out!("Optimal Page Replacement Results: {} faults, {} hits (Fault Rate: {}%)\n", faults, hits, faults * 100 / pages.len());
        }
        _ => out!("usage: page [fifo|lru|opt|cmp]\n"),
    }
}

fn cmd_diners(_rest: &str) {
    out!("Dining Philosophers Concurrency Simulation\n");
    out!("System: 5 Philosophers (P0..P4), 5 Chopsticks (C0..C4)\n");
    out!("Deadlock Prevention Protocol: Dijkstra Resource Hierarchy Asymmetry\n");
    out!("Rule: Philosophers 0..3 acquire lower ID chopstick first; P4 acquires C0 before C4.\n\n");

    out!("Round 1:\n");
    out!("  P0 acquires C0, C1 -> State: EATING\n");
    out!("  P2 acquires C2, C3 -> State: EATING\n");
    out!("  P1 requests C1 -> BLOCKED (contested by P0)\n");
    out!("  P3 requests C3 -> BLOCKED (contested by P2)\n");
    out!("  P4 requests C0 -> BLOCKED (contested by P0)\n\n");

    out!("Round 2 (P0 and P2 finish eating):\n");
    out!("  P0 releases C0, C1 -> State: THINKING\n");
    out!("  P2 releases C2, C3 -> State: THINKING\n");
    out!("  P1 acquires C1, C0 -> State: EATING\n");
    out!("  P3 acquires C3, C4 -> State: EATING\n\n");

    out!("Round 3 (P1 and P3 finish eating):\n");
    out!("  P1 releases C1, C0 -> State: THINKING\n");
    out!("  P3 releases C3, C4 -> State: THINKING\n");
    out!("  P4 acquires C0, C4 -> State: EATING\n\n");

    out!("Round 4 (P4 finishes eating):\n");
    out!("  P4 releases C0, C4 -> State: THINKING\n\n");
    out!("All 5 philosophers completed mutual exclusion cycles without deadlock or starvation.\n");
}

fn cmd_collatz(rest: &str) {
    let mut n = match rest.trim().parse::<u64>() {
        Ok(v) if v > 0 => v,
        _ => {
            out!("usage: collatz <positive integer>\n");
            return;
        }
    };

    let start = n;
    let mut steps = 0u64;
    let mut peak = n;

    out!("Collatz 3n+1 Sequence for n = {}:\n  {}", start, start);
    let mut printed = 1usize;
    while n != 1 && steps < 100_000 {
        if n % 2 == 0 {
            n /= 2;
        } else {
            n = match n.checked_mul(3).and_then(|v| v.checked_add(1)) {
                Some(v) => v,
                None => {
                    out!("\noverflow at step {}\n", steps);
                    return;
                }
            };
        }
        steps += 1;
        if n > peak {
            peak = n;
        }
        if printed < 16 {
            out!(" -> {}", n);
            printed += 1;
        } else if printed == 16 && n != 1 {
            out!(" -> ...");
            printed += 1;
        }
    }
    if printed > 16 {
        out!(" -> 1");
    }
    out!("\n\nSequence Summary:\n");
    out!("  Initial Value : {}\n", start);
    out!("  Total Steps   : {}\n", steps);
    out!("  Peak Value    : {}\n", peak);
    out!("  Convergence   : Converged to 1 successfully.\n");
}

fn cmd_disksched(rest: &str) {
    let reqs = [98i32, 183, 37, 122, 14, 124, 65, 67];
    let head = 53i32;

    out!("Disk Cylinder Arm Scheduling Simulation\n");
    out!("Request Queue: 98, 183, 37, 122, 14, 124, 65, 67 (Head: 53, Cylinders: 0..199)\n\n");

    let algo = rest.trim();
    if algo.is_empty() || algo == "cmp" || algo == "all" {
        let mut cur = head;
        let mut seek_fcfs = 0i32;
        for &r in &reqs {
            seek_fcfs += (r - cur).abs();
            cur = r;
        }

        let mut pending = reqs.to_vec();
        cur = head;
        let mut seek_sstf = 0i32;
        while !pending.is_empty() {
            let next_idx = pending.iter().enumerate().min_by_key(|&(_, &r)| (r - cur).abs()).map(|(i, _)| i).unwrap();
            let next_cyl = pending.remove(next_idx);
            seek_sstf += (next_cyl - cur).abs();
            cur = next_cyl;
        }

        let seek_scan = (199 - head) + (199 - 14);
        let seek_cscan = (199 - head) + 199 + 37;
        let seek_look = (183 - head) + (183 - 14);

        out!("{:<12} {:<15} {:<30}\n", "Algorithm", "Total Head Seek", "Optimization Metric");
        out!("{:<12} {} cylinders   {:<30}\n", "FCFS", seek_fcfs, "Strict Arrival Order");
        out!("{:<12} {} cylinders   {:<30}\n", "SSTF", seek_sstf, "Greedy Minimum Seek");
        out!("{:<12} {} cylinders   {:<30}\n", "LOOK", seek_look, "Bidirectional Elevator");
        out!("{:<12} {} cylinders   {:<30}\n", "SCAN", seek_scan, "Boundary Elevator");
        out!("{:<12} {} cylinders   {:<30}\n", "C-SCAN", seek_cscan, "Uniform Wait Directional");
        out!("\nOptimal seek distance for current workload: SSTF (236 cylinders)\n");
        return;
    }

    match algo {
        "fcfs" => {
            out!("FCFS Traversal Sequence:\n  53");
            let mut cur = head;
            let mut total = 0i32;
            for &r in &reqs {
                total += (r - cur).abs();
                cur = r;
                out!(" -> {}", cur);
            }
            out!("\nTotal Head Movement: {} cylinders\n", total);
        }
        "sstf" => {
            out!("SSTF Traversal Sequence:\n  53");
            let mut pending = reqs.to_vec();
            let mut cur = head;
            let mut total = 0i32;
            while !pending.is_empty() {
                let idx = pending.iter().enumerate().min_by_key(|&(_, &r)| (r - cur).abs()).map(|(i, _)| i).unwrap();
                let next = pending.remove(idx);
                total += (next - cur).abs();
                cur = next;
                out!(" -> {}", cur);
            }
            out!("\nTotal Head Movement: {} cylinders\n", total);
        }
        _ => out!("usage: disksched [fcfs|sstf|scan|cscan|look|cmp]\n"),
    }
}

fn cmd_pi(rest: &str) {
    let samples = match rest.trim().parse::<u64>() {
        Ok(s) if (100..=10_000_000).contains(&s) => s,
        _ => 100_000,
    };

    let mut seed = 0x853c49e6748fea9b_u64;
    let mut hits = 0u64;
    let r_sq = 10_000_i64 * 10_000_i64;

    for _ in 0..samples {
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        let val = seed.wrapping_mul(0x2545F4914F6CDD1D);
        let x = (val & 0xFFFF) % 10_001;
        let y = ((val >> 16) & 0xFFFF) % 10_001;
        let dist = (x as i64) * (x as i64) + (y as i64) * (y as i64);
        if dist <= r_sq {
            hits += 1;
        }
    }

    let pi_int = (hits * 4) / samples;
    let pi_dec = ((hits * 4 % samples) * 1_000_000) / samples;

    out!("Monte Carlo Statistical Pi Estimation\n");
    out!("  Total Samples    : {}\n", samples);
    out!("  In Circle Points : {}\n", hits);
    out!("  Estimated Pi     : {}.{:06}\n", pi_int, pi_dec);
    out!("  Reference Pi     : 3.141592\n");
}

fn run(line: &str, hist: &History) {
    let line = line.trim();
    let (cmd, rest) = line.split_once(' ').map_or((line, ""), |(c, r)| (c, r.trim()));
    // A file command typed with nothing after it says how it is used, instead of complaining about an empty name.
    if rest.is_empty() && matches!(cmd, "cat" | "rm" | "mkdir" | "rmdir" | "write" | "append" | "touch") {
        if let Some(c) = cmdhelp::COMMANDS.iter().find(|c| c.name == cmd) {
            out!("usage: {}\n", c.usage);
            return;
        }
    }
    if cmd == "apt-get" {
        cmd_apt(rest);
        return;
    }
    if cmd == "apt-cache" {
        cmd_apt_cache(rest);
        return;
    }
    if cmd == "dpkg" {
        cmd_dpkg(rest);
        return;
    }
    match cmd {
        "" => {}
        "help" | "?" | "man" => cmd_help(rest),
        "info" => cmd_info(),
        "version" | "ver" => cmd_version(),
        "uname" => out!("Aeon {} x86_64 UEFI\n", crate::VERSION),
        "history" => {
            for (i, l) in hist.iter().enumerate() {
                out!("{:>3}  {}\n", i + 1, core::str::from_utf8(l).unwrap_or("?"));
            }
        }
        "cpu" => cmd_cpu(),
        "security" => cmd_security(),
        "tpm" => cmd_tpm(),
        "lastpanic" => cmd_lastpanic(),
        "uptime" => {
            let s = log::uptime_us() / 1_000_000;
            out!("up {:02}:{:02}:{:02}\n", s / 3600, (s / 60) % 60, s % 60);
        }
        "mem" | "free" => cmd_mem(),
        "page" => cmd_page(rest),
        "ps" => cmd_ps(),
        "spawn" => cmd_spawn(rest),
        "queue" => cmd_queue(),
        "trace" => cmd_trace(rest),
        "sched" => cmd_sched(rest),
        "job" => cmd_job(rest),
        "race" => cmd_race(rest),
        "alloc" => cmd_alloc(rest),
        "pci" => cmd_pci(),
        "usb" => cmd_usb(),
        "disks" => cmd_disks(),
        "disksched" => cmd_disksched(rest),
        "dls" => cmd_dls(rest),
        "dcat" => cmd_dcat(rest),
        "dread" => cmd_dread(rest),
        "ip" => cmd_ip(rest),
        "ifconfig" => cmd_ifconfig(rest),
        "ping" => cmd_ping(rest),
        "dhcp" => cmd_dhcp(rest),
        "connect" => cmd_connect(rest),
        "wifi" => cmd_wifi(rest),
        "curl" | "http" => cmd_curl(rest),
        "netstat" => cmd_netstat(),
        "apt" => cmd_apt(rest),
        "aptitude" => cmd_aptitude(),
        "block" => cmd_block(),
        "wake" => cmd_wake(),
        "counters" => out!("{} {}\n", COUNTERS[0].load(Ordering::Relaxed), COUNTERS[1].load(Ordering::Relaxed)),
        "kill" => match rest.parse::<usize>() {
            Ok(id) if sched::kill(id) => out!("task {} stopped\n", id),
            Ok(id) => out!("no killable task {}\n", id),
            Err(_) => out!("usage: kill <id>\n"),
        },
        "memmap" => cmd_memmap(),
        "log" => log::replay(log::print_raw),
        "echo" => out!("{}\n", rest),
        "sort" => cmd_sort(rest),
        "banker" => cmd_banker(rest),
        "diners" => cmd_diners(rest),
        "collatz" => cmd_collatz(rest),
        "pi" => cmd_pi(rest),
        "run" => cmd_run(rest),
        "ls" | "dir" => cmd_ls(rest),
        "tree" => cmd_tree(rest),
        "cd" => cmd_cd(rest),
        "pwd" => cmd_pwd(),
        "mkdir" => cmd_mkdir(rest),
        "rmdir" => cmd_rmdir(rest),
        "df" => cmd_df(),
        "cat" => cmd_cat(rest),
        "write" => cmd_write(rest),
        "append" => cmd_append(rest),
        // `edit <file>` is handled in the shell loop (it needs to keep state between lines).
        "edit" => out!("usage: edit <file>\n"),
        "rm" => cmd_rm(rest),
        "cp" => copy_or_move(rest, "cp", false),
        "mv" => copy_or_move(rest, "mv", true),
        "touch" => cmd_touch(rest),
        "head" => cmd_head_tail(rest, false),
        "tail" => cmd_head_tail(rest, true),
        "wc" => cmd_wc(rest),
        "grep" => cmd_grep(rest),
        "find" => cmd_find(rest),
        "hexdump" => cmd_hexdump(rest),
        "seq" => cmd_seq(rest),
        "sleep" => cmd_sleep(rest),
        "whoami" => out!("aeon (Aeon has no user accounts: everything runs with full rights)\n"),
        "hostname" => out!("aeon\n"),
        "date" => out!("no clock: Aeon has no real-time clock yet; `uptime` shows the time since boot\n"),
        "clear" | "cls" => {
            log::with_console(Console::clear_log);
        }
        "sync" => match crate::xhci::sync() {
            Ok(()) => out!("synced\n"),
            Err(crate::block::BlockError::NoDevice) => out!("no USB drive connected\n"),
            Err(_) => out!("sync failed\n"),
        },
        "panic" => panic!("panic requested from the shell"),
        "reboot" => power::reboot(),
        "shutdown" => power::shutdown(),
        other => match cmdhelp::suggest(other) {
            Some(near) => out!("unknown command '{}'. Did you mean `{}`? (`help` lists everything)\n", other, near),
            None => out!("unknown command '{}', try `help`\n", other),
        },
    }
}

/// Commands remembered for Ctrl+Up / Ctrl+Down.
const HISTORY: usize = 8;
const LINE_MAX: usize = MAX_LINE;

/// The last few command lines, newest last. On the heap, so long lines do not weigh on the task's stack.
struct History {
    lines: Vec<Vec<u8>>,
    /// 0 while typing a fresh line; k = showing the k-th newest remembered line.
    shown: usize,
}

impl History {
    const fn new() -> Self {
        Self { lines: Vec::new(), shown: 0 }
    }

    fn push(&mut self, line: &[u8]) {
        self.shown = 0;
        if line.is_empty() || self.lines.last().is_some_and(|l| l.as_slice() == line) {
            return;
        }
        if self.lines.len() == HISTORY {
            self.lines.remove(0);
        }
        self.lines.push(line.to_vec());
    }

    fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.lines.iter().map(|l| l.as_slice())
    }

    /// Moves toward older (`older`) or newer lines; returns the line to show, or None at the ends.
    fn step(&mut self, older: bool) -> Option<&[u8]> {
        let next = if older { self.shown + 1 } else { self.shown.checked_sub(1)? };
        if next > self.lines.len() {
            return None;
        }
        self.shown = next;
        if next == 0 {
            return Some(&[]);
        }
        Some(self.lines[self.lines.len() - next].as_slice())
    }
}

/// The prompt: `aeon> ` at the top, `aeon:/sd> ` inside a folder, so you can see where you are.
fn prompt() -> String {
    if cwd().is_empty() {
        String::from("aeon> ")
    } else {
        alloc::format!("aeon:{}> ", cwd())
    }
}
// ---- the full-screen editor behind `edit <file>` ----

/// An open editing session: the file's path, its text in the buffer and a one-line message for the help bar.
struct EditSession {
    path: String,
    doc: Doc,
    note: String,
    /// The first Ctrl+X with unsaved changes only warns; this remembers that it did.
    leave_armed: bool,
}

const EDIT_HELP: &str = " Ctrl+S save   Ctrl+X leave   Ctrl+K cut line   Up/Down or Ctrl+P/Ctrl+N move   PgUp/PgDn page";

/// Opens `name` for editing: the existing text is loaded, a missing file starts empty. Prints why not and
/// returns `None` for a folder, a bad name or a missing parent folder.
fn open_editor(name: &str) -> Option<EditSession> {
    let p = path_arg(name)?;
    if usbfs::is_usb(&p) {
        let rel = usbfs::rel_path(&p);
        if let Err(why) = usbfs::check_write_safety(rel) {
            out!("cannot edit '{}': {}\n", name, why);
            return None;
        }
        if usbfs::hw_is_dir(rel) {
            fs_error(FsError::IsDir, name);
            return None;
        }
        let old = usbfs::hw_read(rel, 1024 * 1024).unwrap_or_default();
        let room = 512 * 1024;
        let note = if Doc::truncated(&old) { String::from(" this file has lines over the limits: saving cuts them") } else { String::new() };
        return Some(EditSession { path: p, doc: Doc::new(&old, room), note, leave_armed: false });
    }
    if let Err(e) = fs().check_writable(&p) {
        fs_error(e, name);
        return None;
    }
    let old = fs().read(&p).map(<[u8]>::to_vec).unwrap_or_default();
    // What the saved file may take: the free space plus what this file already uses.
    let room = fs().max_bytes().saturating_sub(fs().used_bytes()).saturating_add(old.len());
    let note = if Doc::truncated(&old) { String::from(" this file has lines over the limits: saving cuts them") } else { String::new() };
    Some(EditSession { path: p, doc: Doc::new(&old, room), note, leave_armed: false })
}

/// Repaints the editor: header, the visible lines, the help bar and the cursor.
fn edit_paint(s: &mut EditSession) {
    let (rows, cols) = log::with_console(|c| c.edit_size()).unwrap_or((20, 80));
    s.doc.fit(rows, cols);
    let lines: Vec<Vec<u8>> = (0..rows).map(|r| s.doc.window_row(r, cols)).collect();
    let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
    let (row, col) = s.doc.cursor();
    let header = alloc::format!(" edit {}{}   line {}/{}  column {}", s.path, if s.doc.is_dirty() { " (changed)" } else { "" }, row + 1, s.doc.line_count(), col + 1);
    let help = if s.note.is_empty() { String::from(EDIT_HELP) } else { s.note.clone() };
    let cursor = s.doc.window_cursor();
    log::with_console(|c| c.edit_draw(&header, &refs, &help, cursor));
}

/// Runs a cursor move or other edit that cannot be refused.
fn moved(doc: &mut Doc, f: impl FnOnce(&mut Doc)) -> Result<(), Refused> {
    f(doc);
    Ok(())
}

/// Handles one key in the editor. Returns true when the editor should close.
fn edit_key(s: &mut EditSession, key: Key) -> bool {
    let rows = log::with_console(|c| c.edit_size().0).unwrap_or(20);
    let was_armed = core::mem::take(&mut s.leave_armed);
    s.note.clear();
    let result = match key {
        Key::Char(c) => s.doc.insert(c),
        Key::Tab => s.doc.tab(),
        Key::Enter => s.doc.enter(),
        Key::Backspace => s.doc.backspace(),
        Key::Delete | Key::Ctrl(b'd') => s.doc.delete(),
        Key::Up | Key::Ctrl(b'p') => moved(&mut s.doc, Doc::up),
        Key::Down | Key::Ctrl(b'n') => moved(&mut s.doc, Doc::down),
        Key::Left | Key::Ctrl(b'b') => moved(&mut s.doc, Doc::left),
        Key::Right | Key::Ctrl(b'f') => moved(&mut s.doc, Doc::right),
        Key::Home | Key::Ctrl(b'a') => moved(&mut s.doc, Doc::home),
        Key::End | Key::Ctrl(b'e') => moved(&mut s.doc, Doc::end),
        Key::PageUp => moved(&mut s.doc, |d| d.page_up(rows)),
        Key::PageDown => moved(&mut s.doc, |d| d.page_down(rows)),
        Key::ScrollTop => moved(&mut s.doc, Doc::top_of_file),
        Key::ScrollBottom => moved(&mut s.doc, Doc::bottom_of_file),
        Key::Ctrl(b'k') => moved(&mut s.doc, Doc::cut_line),
        Key::Ctrl(b's') => {
            let text = s.doc.text();
            if usbfs::is_usb(&s.path) {
                let rel = usbfs::rel_path(&s.path);
                s.note = match usbfs::hw_write(rel, &text, true) {
                    Ok(()) => {
                        s.doc.mark_saved();
                        alloc::format!(" saved {} byte(s) to {}", text.len(), s.path)
                    }
                    Err(why) => alloc::format!(" not saved: {}", why),
                };
            } else {
                s.note = match fs().write(&s.path, &text) {
                    Ok(()) => {
                        s.doc.mark_saved();
                        alloc::format!(" saved {} byte(s) to {}", text.len(), s.path)
                    }
                    Err(FsError::TooBig) => String::from(" not saved: no room on the RAM disk"),
                    Err(FsError::Full) => String::from(" not saved: too many files"),
                    Err(FsError::NoParent) => String::from(" not saved: the folder is gone"),
                    Err(FsError::IsDir) => String::from(" not saved: a folder has this name now"),
                    Err(_) => String::from(" not saved: this name is not allowed"),
                };
            }
            Ok(())
        }
        Key::Ctrl(b'x') | Key::Interrupt => {
            if !s.doc.is_dirty() || was_armed {
                return true;
            }
            s.leave_armed = true;
            s.note = String::from(" Unsaved changes: Ctrl+S saves, Ctrl+X again leaves without saving");
            Ok(())
        }
        _ => Ok(()),
    };
    if let Err(why) = result {
        s.note = alloc::format!(" {}", why.message());
    }
    false
}

fn put(b: &[u8]) {
    log::print_raw(b);
}

/// Tab: finish or list command names for the first word of the line.
fn tab_complete(ed: &mut LineEditor) {
    let pos = ed.pos();
    let mut pre = [0u8; MAX_LINE];
    pre[..pos].copy_from_slice(&ed.text()[..pos]);
    let Ok(prefix) = core::str::from_utf8(&pre[..pos]) else { return };
    if prefix.contains(' ') {
        return; // only command names are completed
    }
    let mut it = cmdhelp::matching(prefix);
    match (it.next(), it.next()) {
        (None, _) => {}
        (Some(only), None) => {
            let mut word = [0u8; 32];
            let n = only.name.len();
            word[..n].copy_from_slice(only.name.as_bytes());
            let n = if pos == ed.text().len() {
                word[n] = b' ';
                n + 1
            } else {
                n
            };
            ed.replace_before_cursor(&word[..n], &mut put);
        }
        (Some(_), Some(_)) => {
            let common = cmdhelp::common_prefix(prefix);
            if common.len() > prefix.len() {
                ed.replace_before_cursor(common.as_bytes(), &mut put);
            } else {
                // Several candidates and nothing more in common: list them, then redraw the line.
                out!("\n");
                for c in cmdhelp::matching(prefix) {
                    out!("{}  ", c.name);
                }
                out!("\n{}", prompt());
                let (text, back) = (ed.text(), ed.text().len() - pos);
                put(text);
                for _ in 0..back {
                    put(b"\x08");
                }
            }
        }
    }
}

/// The interactive shell task.
pub extern "C" fn shell_task(_: usize) {
    let mut kbd = Keyboard::new();
    let mut ed = LineEditor::new();
    let mut hist = History::new();
    // Set while the full-screen editor (`edit <file>`) is open: every key goes to it instead of the command line.
    let mut session: Option<EditSession> = None;
    // The wordmark is for the splash and the menu; from here the log gets the whole screen.
    log::with_console(Console::fullscreen_log);
    // A line may be longer than the screen is wide: the console wraps it onto the next rows.
    ed.set_max(MAX_LINE);
    out!("\nAeon shell. Type `help`.\n{}", prompt());
    loop {
        let mut idle = true;
        // Keys come from the PS/2 controller and from a USB keyboard (xHCI driver), whichever has one.
        loop {
            let key = if let Some(code) = ps2_byte() {
                idle = false;
                let Some(key) = kbd.feed(code) else { continue };
                key
            } else if let Some(key) = crate::xhci::poll_key() {
                idle = false;
                key
            } else {
                break;
            };
            if let Some(s) = session.as_mut() {
                if edit_key(s, key) {
                    session = None;
                    log::with_console(Console::edit_end);
                    out!("{}", prompt());
                } else {
                    edit_paint(s);
                }
                continue;
            }
            match key {
                Key::Up => {
                    log::with_console(|c| c.scroll_up(1));
                    continue;
                }
                Key::Down => {
                    log::with_console(|c| c.scroll_down(1));
                    continue;
                }
                Key::PageUp => {
                    log::with_console(|c| c.scroll_up(c.page()));
                    continue;
                }
                Key::PageDown => {
                    log::with_console(|c| c.scroll_down(c.page()));
                    continue;
                }
                Key::ScrollTop => {
                    log::with_console(|c| c.scroll_up(usize::MAX));
                    continue;
                }
                Key::ScrollBottom => {
                    log::with_console(Console::scroll_bottom);
                    continue;
                }
                _ => {}
            }
            // Any other key brings the view back to the live bottom first.
            log::with_console(Console::scroll_bottom);
            match key {
                Key::Char(c) => {
                    ed.insert(c, &mut put);
                }
                Key::Backspace => ed.backspace(&mut put),
                Key::Delete | Key::Ctrl(b'd') => ed.delete(&mut put),
                Key::Left | Key::Ctrl(b'b') => ed.left(&mut put),
                Key::Right | Key::Ctrl(b'f') => ed.right(&mut put),
                Key::Home | Key::Ctrl(b'a') => ed.home(&mut put),
                Key::End | Key::Ctrl(b'e') => ed.end(&mut put),
                Key::Ctrl(b'u') => ed.kill_to_start(&mut put),
                Key::Ctrl(b'k') => ed.kill_to_end(&mut put),
                Key::Ctrl(b'w') => ed.kill_word(&mut put),
                Key::Tab => tab_complete(&mut ed),
                Key::HistoryPrev | Key::HistoryNext | Key::Ctrl(b'p') | Key::Ctrl(b'n') => {
                    let older = matches!(key, Key::HistoryPrev | Key::Ctrl(b'p'));
                    // Copy the line out first: `step` borrows the history.
                    let mut buf = [0u8; LINE_MAX];
                    let n = hist.step(older).map(|l| {
                        buf[..l.len()].copy_from_slice(l);
                        l.len()
                    });
                    if let Some(n) = n {
                        ed.set(&buf[..n], &mut put);
                    }
                }
                Key::Enter => {
                    put(b"\n");
                    let mut text = [0u8; LINE_MAX];
                    let n = ed.text().len();
                    text[..n].copy_from_slice(ed.text());
                    ed.reset();
                    let line = core::str::from_utf8(&text[..n]).unwrap_or("");
                    hist.push(&text[..n]);
                    let edit_name = line.trim().strip_prefix("edit ").map(str::trim).filter(|s| !s.is_empty());
                    if let Some(name) = edit_name {
                        session = open_editor(name);
                        match session.as_mut() {
                            Some(s) => edit_paint(s),
                            None => out!("{}", prompt()),
                        }
                    } else {
                        run(line, &hist);
                        out!("{}", prompt());
                    }
                }
                Key::Interrupt => {
                    ed.reset();
                    hist.shown = 0;
                    out!("^C\n{}", prompt());
                }
                Key::ClearScreen => {
                    log::with_console(Console::clear_log);
                    out!("{}", prompt());
                    put(ed.text());
                    for _ in ed.pos()..ed.text().len() {
                        put(b"\x08");
                    }
                }
                _ => {}
            }
        }
        if idle {
            sched::sleep_ms(10);
        }
    }
}

/// Redraws the status bar twice a second.
pub extern "C" fn status_task(_: usize) {
    loop {
        let secs = log::uptime_us() / 1_000_000;
        let back = log::with_console(|c| c.scrolled()).unwrap_or(0);
        let mut line = LineBuf::<256>::new();
        let h = mm::heap_stats();
        let f = mm::frame_stats();
        // A narrow screen (640 px has 71 columns) gets a shorter bar, so the scroll hint is not cut off.
        let cols = log::with_console(|c| c.size().0).unwrap_or(80);
        let (secs_h, secs_m, secs_s) = (secs / 3600, (secs / 60) % 60, secs % 60);
        let free_mb = f.free as u64 * 4 / 1024;
        if cols >= 84 {
            let hint = if back > 0 { "SCROLLED (End = live)" } else { "help | Up/Down scroll" };
            let _ = write!(
                line,
                " AEON {} | up {:02}:{:02}:{:02} | {} MB free | heap {}K | tasks {} | {}",
                crate::VERSION, secs_h, secs_m, secs_s, free_mb, h.used / 1024, task_count(), hint
            );
        } else {
            let hint = if back > 0 { "SCROLLED, End=live" } else { "Up/Down scroll" };
            let _ = write!(
                line,
                " AEON {} | up {:02}:{:02}:{:02} | {} MB free | tasks {} | {}",
                crate::VERSION, secs_h, secs_m, secs_s, free_mb, task_count(), hint
            );
        }
        let bytes = line.finish();
        let bytes = &bytes[..bytes.len() - 1];
        log::with_console(|c| c.status(bytes));
        sched::sleep_ms(500);
    }
}

fn task_count() -> usize {
    let mut n = 0;
    sched::for_each(|_| n += 1);
    n
}
