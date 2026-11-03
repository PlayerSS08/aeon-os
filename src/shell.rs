//! Interactive kernel shell and the status-bar task. Both run as scheduler tasks.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::acpi;
use crate::arch;
use crate::boot;
use crate::cmdhelp;
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

/// The RAM filesystem. Only the shell task touches it, so no second reference to it ever exists.
static FS: Global<RamFs> = Global::new(RamFs::new(64, 1 << 20));

fn fs() -> &'static mut RamFs {
    // SAFETY: only the single shell task calls this, and no result is held across another command.
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
/// is a `yield`, not a timer preemption. Compare `worker`, which never yields (CA-6 layer 6).
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
/// ends (so the task goes through ready, running and terminated, CA-6 layer 5).
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

/// The shared-resource problem (CA-6 layer 7). Two tasks each add 1 to one counter `n` times. The add is
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
    if fs().is_dir(&p) {
        *cwd() = if p == "/" { String::new() } else { p };
    } else if fs().read(&p).is_some() {
        fs_error(FsError::NotDir, arg);
    } else {
        out!("no such folder '{}'\n", arg);
    }
}

fn cmd_mkdir(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    match fs().mkdir(&p) {
        Ok(()) => out!("made folder {}\n", p),
        Err(e) => fs_error(e, arg),
    }
}

fn cmd_rmdir(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
    match fs().rmdir(&p) {
        Ok(()) => {
            out!("removed folder {}\n", p);
            // Never leave the shell standing in a folder that no longer exists.
            if !fs().is_dir(cwd().as_str()) && !cwd().is_empty() {
                cwd().clear();
            }
        }
        Err(e) => fs_error(e, arg),
    }
}

fn cmd_ls(arg: &str) {
    let Some(p) = folder_arg(arg) else { return };
    if !fs().is_dir(&p) {
        match fs().read(&p) {
            Some(d) => out!("{:>8}  {}\n", d.len(), arg),
            None => out!("no such folder '{}'\n", arg),
        }
        return;
    }
    let mut n = 0;
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

fn cmd_tree(arg: &str) {
    let Some(p) = folder_arg(arg) else { return };
    if !fs().is_dir(&p) {
        out!("no such folder '{}'\n", arg);
        return;
    }
    out!("{}\n", p);
    print_tree(&p, 2, 0);
}

fn cmd_df() {
    let f = fs();
    out!("{} file(s), used {} of {} bytes", f.count(), f.used_bytes(), f.max_bytes());
    if f.dir_count() > 0 {
        out!(", {} folder(s)", f.dir_count());
    }
    out!("\n");
}

fn cmd_cat(arg: &str) {
    let Some(p) = path_arg(arg) else { return };
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
    let (name, text) = args.split_once(' ').map_or((args, ""), |(n, t)| (n, t.trim_start()));
    let Some(p) = path_arg(name) else { return };
    let data = unescape(text);
    match fs().write(&p, &data) {
        Ok(()) => out!("wrote {} byte(s) to {}\n", data.len(), name),
        Err(e) => fs_error(e, name),
    }
}

/// `append <file> <text>`: adds the text and a new line to the end of the file.
fn cmd_append(args: &str) {
    let (name, text) = args.split_once(' ').map_or((args, ""), |(n, t)| (n, t.trim_start()));
    let Some(p) = path_arg(name) else { return };
    let mut data = unescape(text);
    data.push(b'\n');
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

fn run(line: &str, hist: &History) {
    let line = line.trim();
    let (cmd, rest) = line.split_once(' ').map_or((line, ""), |(c, r)| (c, r.trim()));
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
        "ps" => cmd_ps(),
        "spawn" => cmd_spawn(rest),
        "queue" => cmd_queue(),
        "trace" => cmd_trace(rest),
        "job" => cmd_job(rest),
        "race" => cmd_race(rest),
        "alloc" => cmd_alloc(rest),
        "pci" => cmd_pci(),
        "usb" => cmd_usb(),
        "disks" => cmd_disks(),
        "dls" => cmd_dls(rest),
        "dcat" => cmd_dcat(rest),
        "dread" => cmd_dread(rest),
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
        "clear" | "cls" => {
            log::with_console(Console::clear_log);
        }
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

/// The last few command lines, newest last.
struct History {
    lines: [[u8; LINE_MAX]; HISTORY],
    lens: [usize; HISTORY],
    count: usize,
    /// 0 while typing a fresh line; k = showing the k-th newest remembered line.
    shown: usize,
}

impl History {
    const fn new() -> Self {
        Self { lines: [[0; LINE_MAX]; HISTORY], lens: [0; HISTORY], count: 0, shown: 0 }
    }

    fn push(&mut self, line: &[u8]) {
        self.shown = 0;
        if line.is_empty() || (self.count > 0 && &self.lines[self.count - 1][..self.lens[self.count - 1]] == line) {
            return;
        }
        if self.count == HISTORY {
            self.lines.copy_within(1.., 0);
            self.lens.copy_within(1.., 0);
            self.count -= 1;
        }
        self.lines[self.count][..line.len()].copy_from_slice(line);
        self.lens[self.count] = line.len();
        self.count += 1;
    }

    fn iter(&self) -> impl Iterator<Item = &[u8]> {
        (0..self.count).map(|i| &self.lines[i][..self.lens[i]])
    }

    /// Moves toward older (`older`) or newer lines; returns the line to show, or None at the ends.
    fn step(&mut self, older: bool) -> Option<&[u8]> {
        let next = if older { self.shown + 1 } else { self.shown.checked_sub(1)? };
        if next > self.count {
            return None;
        }
        self.shown = next;
        if next == 0 {
            return Some(&[]);
        }
        let i = self.count - next;
        Some(&self.lines[i][..self.lens[i]])
    }
}

const PROMPT: &str = "aeon> ";
const EDIT_PROMPT: &str = "   | ";

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
                out!("\n{}", PROMPT);
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
    // Set while `edit <file>` is collecting lines: (file name, text so far).
    let mut editing: Option<(String, Vec<u8>)> = None;
    // The wordmark is for the splash and the menu; from here the log gets the whole screen.
    log::with_console(Console::fullscreen_log);
    // A line may be longer than the screen is wide: the console wraps it onto the next rows.
    ed.set_max(MAX_LINE);
    out!("\nAeon shell. Type `help`.\n{}", PROMPT);
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
                    if let Some(doc) = editing.as_mut() {
                        // Inside `edit`: a lone "." saves, anything else is one more line.
                        if line == "." {
                            let (name, buf) = editing.take().unwrap_or_default();
                            match fs().write(&name, &buf) {
                                Ok(()) => out!("saved {} byte(s) to {}\n", buf.len(), name),
                                Err(e) => fs_error(e, &name),
                            }
                            out!("{}", PROMPT);
                        } else if doc.1.len() + n + 1 > fs().max_bytes() {
                            editing = None;
                            out!("too much text for the RAM disk: edit cancelled, nothing saved\n{}", PROMPT);
                        } else {
                            doc.1.extend_from_slice(&text[..n]);
                            doc.1.push(b'\n');
                            out!("{}", EDIT_PROMPT);
                        }
                        continue;
                    }
                    hist.push(&text[..n]);
                    let edit_name = line.trim().strip_prefix("edit ").map(str::trim).filter(|s| !s.is_empty());
                    if let Some(name) = edit_name {
                        let target = path_arg(name).filter(|p| match fs().check_writable(p) {
                            Ok(()) => true,
                            Err(e) => {
                                fs_error(e, name);
                                false
                            }
                        });
                        if let Some(p) = target {
                            out!("editing {}: type lines, a line with only . saves, Ctrl+C cancels\n{}", p, EDIT_PROMPT);
                            editing = Some((p, Vec::new()));
                        } else {
                            out!("{}", PROMPT);
                        }
                    } else {
                        run(line, &hist);
                        out!("{}", PROMPT);
                    }
                }
                Key::Interrupt => {
                    ed.reset();
                    hist.shown = 0;
                    if editing.take().is_some() {
                        out!("^C edit cancelled, nothing saved\n{}", PROMPT);
                    } else {
                        out!("^C\n{}", PROMPT);
                    }
                }
                Key::ClearScreen => {
                    log::with_console(Console::clear_log);
                    out!("{}", PROMPT);
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
