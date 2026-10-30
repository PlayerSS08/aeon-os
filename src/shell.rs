//! Interactive kernel shell and the status-bar task. Both run as scheduler tasks.

use alloc::vec::Vec;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch;
use crate::boot;
use crate::fb::Console;
use crate::keymap::{Key, Keyboard};
use crate::log;
use crate::logfmt::LineBuf;
use crate::memmap;
use crate::mm;
use crate::power;
use crate::sched;

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

const HELP: &str = "commands:
  help               this list
  info               firmware, CPU clock, Secure Boot
  version            Aeon version and revision
  security           which protections are active right now
  tpm                measured-boot PCR values
  lastpanic          how the previous boot ended
  uptime             time since boot
  mem                frames, heap and RAM totals
  ps                 list tasks
  spawn              start 2 busy workers (watch `ps` and `counters`)
  counters           worker progress
  kill <id>          stop a task
  memmap             firmware memory map
  log                replay the boot log
  echo <text>        print text
  sort <n> <n> ...   sort numbers (uses the heap)
  clear              clear the log area (or Ctrl+L)
  panic              trigger a kernel panic
  reboot | shutdown
";

fn cmd_info() {
    let b = boot::info();
    out!("firmware   {} rev {:#x}, UEFI {}.{}\n", b.fw_vendor_str(), b.fw_revision, b.uefi_revision >> 16, (b.uefi_revision & 0xFFFF) / 10);
    out!("secure boot {}\n", b.secure_boot.name());
    out!("tsc        {} MHz\n", log::tsc_hz() / 1_000_000);
    out!("image      base {:#x} size {} KiB\n", b.image_base, b.image_size / 1024);
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
        yes(b.seed_sources > 0 || b.seed_persisted)
    );
    out!("previous boot      {}\n", if b.last_panic_len == 0 { "ended normally" } else { "ended in a panic or exception, see `lastpanic`" });
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
    out!("ring 3             not implemented, everything runs in the kernel\n");
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

fn cmd_spawn() {
    for (i, name) in ["worker-a", "worker-b"].into_iter().enumerate() {
        match sched::spawn(name, worker, i) {
            Some(id) => out!("started {} as task {}\n", name, id),
            None => out!("cannot start {}: no free task slot or memory\n", name),
        }
    }
}

fn run(line: &str) {
    let line = line.trim();
    let (cmd, rest) = line.split_once(' ').map_or((line, ""), |(c, r)| (c, r.trim()));
    match cmd {
        "" => {}
        "help" | "?" => out!("{}", HELP),
        "info" => cmd_info(),
        "version" => cmd_version(),
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
        "clear" => {
            log::with_console(Console::clear_log);
        }
        "panic" => panic!("panic requested from the shell"),
        "reboot" => power::reboot(),
        "shutdown" => power::shutdown(),
        other => out!("unknown command '{}', try `help`\n", other),
    }
}

/// The interactive shell task.
pub extern "C" fn shell_task(_: usize) {
    const PROMPT: &str = "aeon> ";
    let mut kbd = Keyboard::new();
    let mut line = [0u8; 120];
    let mut len = 0usize;
    out!("\nAeon shell. Type `help`.\n{}", PROMPT);
    loop {
        let mut idle = true;
        while let Some(code) = ps2_byte() {
            idle = false;
            let Some(key) = kbd.feed(code) else { continue };
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
                Key::Enter => {
                    log::print_raw(b"\n");
                    let text = core::str::from_utf8(&line[..len]).unwrap_or("");
                    run(text);
                    len = 0;
                    out!("{}", PROMPT);
                }
                Key::Interrupt => {
                    len = 0;
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
        let mut line = LineBuf::<256>::new();
        let h = mm::heap_stats();
        let f = mm::frame_stats();
        let _ = write!(
            line,
            " AEON {} | up {:02}:{:02}:{:02} | {} MB free | heap {}K | tasks {} | type `help`",
            crate::VERSION,
            secs / 3600,
            (secs / 60) % 60,
            secs % 60,
            f.free as u64 * 4 / 1024,
            h.used / 1024,
            task_count()
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
