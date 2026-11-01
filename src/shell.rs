//! Interactive kernel shell and the status-bar task. Both run as scheduler tasks.

use alloc::vec::Vec;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::acpi;
use crate::arch;
use crate::boot;
use crate::cmdhelp;
use crate::fb::Console;
use crate::global::Global;
use crate::keymap::{Key, Keyboard};
use crate::log;
use crate::logfmt::LineBuf;
use crate::memmap;
use crate::mm;
use crate::power;
use crate::ramfs::{FsError, RamFs, MAX_NAME};
use crate::sched;
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
            out!("Keys: PgUp/PgDn scroll, Home/End top/bottom, Up/Down recall commands, Ctrl+L clear, Ctrl+C cancel.\n\n");
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
    match crate::vblk::with_disk(|d| crate::block::BlockDev::sectors(d)) {
        Some(n) => out!("vda  virtio-blk  {} sectors  {} KiB\n", n, n / 2),
        None => out!("no disk\n"),
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
    match crate::vblk::with_disk(|d| crate::block::BlockDev::read(d, lba, &mut sector)) {
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
fn with_volume<R>(f: impl FnOnce(&crate::fat::Volume, &mut crate::vblk::VirtioBlk) -> R) -> Option<R> {
    let r = crate::vblk::with_disk(|d| match crate::fat::Volume::open_any(d) {
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

fn cmd_spawn() {
    for (i, name) in ["worker-a", "worker-b"].into_iter().enumerate() {
        match sched::spawn(name, worker, i) {
            Some(id) => out!("started {} as task {}\n", name, id),
            None => out!("cannot start {}: no free task slot or memory\n", name),
        }
    }
}

fn cmd_ls() {
    let mut n = 0;
    for (name, size) in fs().names() {
        out!("{:>8}  {}\n", size, name);
        n += 1;
    }
    if n == 0 {
        out!("(no files)\n");
    }
}

fn cmd_df() {
    let f = fs();
    out!("{} file(s), used {} of {} bytes\n", f.count(), f.used_bytes(), f.max_bytes());
}

fn cmd_cat(name: &str) {
    match fs().read(name) {
        Some(data) => {
            log::print_raw(data);
            out!("\n");
        }
        None => out!("no such file '{}'\n", name),
    }
}

fn cmd_write(args: &str) {
    let (name, text) = args.split_once(' ').map_or((args, ""), |(n, t)| (n, t.trim_start()));
    match fs().write(name, text.as_bytes()) {
        Ok(()) => out!("wrote {} byte(s) to {}\n", text.len(), name),
        Err(FsError::BadName) => out!("bad name '{}' (1-{} chars, no / or control characters)\n", name, MAX_NAME),
        Err(FsError::Full) => out!("filesystem full: too many files\n"),
        Err(FsError::TooBig) => out!("filesystem full: out of space\n"),
    }
}

fn cmd_rm(name: &str) {
    if fs().remove(name) {
        out!("removed {}\n", name);
    } else {
        out!("no such file '{}'\n", name);
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

fn run(line: &str) {
    let line = line.trim();
    let (cmd, rest) = line.split_once(' ').map_or((line, ""), |(c, r)| (c, r.trim()));
    match cmd {
        "" => {}
        "help" | "?" => cmd_help(rest),
        "info" => cmd_info(),
        "version" => cmd_version(),
        "cpu" => cmd_cpu(),
        "security" => cmd_security(),
        "tpm" => cmd_tpm(),
        "lastpanic" => cmd_lastpanic(),
        "uptime" => {
            let s = log::uptime_us() / 1_000_000;
            out!("up {:02}:{:02}:{:02}\n", s / 3600, (s / 60) % 60, s % 60);
        }
        "mem" => cmd_mem(),
        "ps" => cmd_ps(),
        "spawn" => cmd_spawn(),
        "pci" => cmd_pci(),
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
        "ls" => cmd_ls(),
        "df" => cmd_df(),
        "cat" => cmd_cat(rest),
        "write" => cmd_write(rest),
        "rm" => cmd_rm(rest),
        "clear" => {
            log::with_console(Console::clear_log);
        }
        "panic" => panic!("panic requested from the shell"),
        "reboot" => power::reboot(),
        "shutdown" => power::shutdown(),
        other => out!("unknown command '{}', try `help`\n", other),
    }
}

/// Commands remembered for Up/Down.
const HISTORY: usize = 8;
const LINE_MAX: usize = 120;

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

/// Erases `len` typed characters on screen and types `new` in their place.
fn replace_input(line: &mut [u8; LINE_MAX], len: &mut usize, new: &[u8]) {
    for _ in 0..*len {
        log::print_raw(b"\x08 \x08");
    }
    let n = new.len().min(LINE_MAX);
    line[..n].copy_from_slice(&new[..n]);
    *len = n;
    log::print_raw(&line[..n]);
}

/// The interactive shell task.
pub extern "C" fn shell_task(_: usize) {
    const PROMPT: &str = "aeon> ";
    let mut kbd = Keyboard::new();
    let mut line = [0u8; LINE_MAX];
    let mut len = 0usize;
    let mut hist = History::new();
    // The wordmark is for the splash and the menu; from here the log gets the whole screen.
    log::with_console(Console::fullscreen_log);
    out!("\nAeon shell. Type `help`.\n{}", PROMPT);
    loop {
        let mut idle = true;
        while let Some(code) = ps2_byte() {
            idle = false;
            let Some(key) = kbd.feed(code) else { continue };
            match key {
                Key::PageUp => {
                    log::with_console(|c| c.scroll_up(c.page()));
                    continue;
                }
                Key::PageDown => {
                    log::with_console(|c| c.scroll_down(c.page()));
                    continue;
                }
                Key::Home => {
                    log::with_console(|c| c.scroll_up(usize::MAX));
                    continue;
                }
                Key::End => {
                    log::with_console(Console::scroll_bottom);
                    continue;
                }
                _ => {}
            }
            // Any other key brings the view back to the live bottom first.
            log::with_console(Console::scroll_bottom);
            match key {
                Key::Char(c) if len < line.len() => {
                    line[len] = c;
                    len += 1;
                    log::print_raw(&[c]);
                }
                Key::Backspace if len > 0 => {
                    len -= 1;
                    log::print_raw(b"\x08 \x08");
                }
                Key::Up | Key::Down => {
                    // Copy the line out first: `step` borrows the history.
                    let mut buf = [0u8; LINE_MAX];
                    let n = hist.step(key == Key::Up).map(|l| {
                        buf[..l.len()].copy_from_slice(l);
                        l.len()
                    });
                    if let Some(n) = n {
                        replace_input(&mut line, &mut len, &buf[..n]);
                    }
                }
                Key::Enter => {
                    log::print_raw(b"\n");
                    hist.push(&line[..len]);
                    let text = core::str::from_utf8(&line[..len]).unwrap_or("");
                    run(text);
                    len = 0;
                    out!("{}", PROMPT);
                }
                Key::Interrupt => {
                    len = 0;
                    hist.shown = 0;
                    out!("^C\n{}", PROMPT);
                }
                Key::ClearScreen => {
                    log::with_console(Console::clear_log);
                    out!("{}", PROMPT);
                    log::print_raw(&line[..len]);
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
        let _ = write!(
            line,
            " AEON {} | up {:02}:{:02}:{:02} | {} MB free | heap {}K | tasks {} | {}",
            crate::VERSION,
            secs / 3600,
            (secs / 60) % 60,
            secs % 60,
            f.free as u64 * 4 / 1024,
            h.used / 1024,
            task_count(),
            if back > 0 { "SCROLLED (End = live)" } else { "help | PgUp/PgDn scroll" }
        );
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
