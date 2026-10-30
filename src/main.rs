//! Aeon boot flow.
//!
//! firmware entry -> early init -> platform discovery -> ExitBootServices -> kernel environment
//! (GDT/IDT, page tables, CPU protections, own stack) -> `kernel_main`.

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

use core::ffi::c_void;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use aeon::acpi::{self, Rsdp};
use aeon::arch;
use aeon::bootinfo::{BootInfo, FbInfo, SecureBoot, MAX_REGIONS};
use aeon::bootopts::{self, BootOpts};
use aeon::cpuid;
use aeon::entropy;
use aeon::fb::Console;
use aeon::splash;
use aeon::tcg2;
use aeon::log;
use aeon::global::Global;
use aeon::logfmt::{self, Level};
use aeon::memmap;
use aeon::mm;
use aeon::paging;
use aeon::sched;
use aeon::shell;
use aeon::pixel::PixelFormat;
use aeon::uefi::{
    self, BootServices, Gop, GopModeInfo, Guid, Handle, InputKey, LoadedImage, Rng, RuntimeServices, Status, SystemTable,
    Tcg2,
};
use aeon::{debug, error, info, warn};

// ---------------------------------------------------------------------------------------------
// Kernel-owned state
// ---------------------------------------------------------------------------------------------

const KSTACK_SIZE: usize = 64 * 1024;

#[repr(C, align(4096))]
struct KernelStack([u8; KSTACK_SIZE]);

/// The kernel stack. Its lowest page is unmapped by `paging::build` as a guard page.
static KSTACK: Global<KernelStack> = Global::new(KernelStack([0; KSTACK_SIZE]));

/// Boot stages that move the splash progress bar: firmware-entry, early-init, platform-discovery,
/// security-init, exit-boot-services, kernel-env, kernel-memory, kernel-tasks.
const TOTAL_STAGES: usize = 8;

struct Stages {
    n: usize,
    items: [(&'static str, u64); 12],
}

static STAGES: Global<Stages> = Global::new(Stages { n: 0, items: [("", 0); 12] });

fn stage(name: &'static str) {
    // SAFETY: single-threaded boot; the reference is dropped before logging.
    let done = unsafe {
        let s = STAGES.get();
        if s.n < s.items.len() {
            s.items[s.n] = (name, arch::rdtsc());
            s.n += 1;
        }
        s.n - 1
    };
    let _ = log::with_console(|c| c.progress(splash::stage_permille(done, TOTAL_STAGES)));
    info!("stage", "enter {}", name);
}

fn boot_info() -> &'static mut BootInfo {
    aeon::boot::info()
}

// ---------------------------------------------------------------------------------------------
// Firmware helpers (valid only until ExitBootServices)
// ---------------------------------------------------------------------------------------------

/// Measures the TSC against the firmware's `Stall` timer. Returns Hz, or 0 if implausible.
fn calibrate_tsc(bs: &BootServices) -> u64 {
    let t0 = arch::rdtsc();
    (bs.stall)(20_000);
    let hz = arch::rdtsc().wrapping_sub(t0).saturating_mul(50);
    if (200_000_000..=10_000_000_000).contains(&hz) {
        hz
    } else {
        0
    }
}

fn pick_gop_mode(gop: &Gop) -> Option<u32> {
    // SAFETY: `gop.mode` is set by firmware and valid for the protocol's lifetime.
    let (max_mode, current) = unsafe { ((*gop.mode).max_mode, (*gop.mode).mode) };
    // The firmware's current mode is normally the panel's native one: keep it when it has a linear
    // framebuffer, so the image is sharp instead of scaled by the display.
    let mut size = 0usize;
    let mut cur: *const GopModeInfo = core::ptr::null();
    if (gop.query_mode)(gop as *const Gop as *mut Gop, current, &mut size, &mut cur) == uefi::SUCCESS && !cur.is_null() {
        // SAFETY: query_mode succeeded, so `cur` points at a firmware-allocated GopModeInfo.
        let i = unsafe { &*cur };
        if i.pixel_format != 3 && i.horizontal_resolution >= 640 && i.vertical_resolution >= 480 {
            return Some(current);
        }
    }
    let mut best: Option<(u32, u64)> = None;
    for m in 0..max_mode {
        let mut size = 0usize;
        let mut info: *const GopModeInfo = core::ptr::null();
        if (gop.query_mode)(gop as *const Gop as *mut Gop, m, &mut size, &mut info) != uefi::SUCCESS || info.is_null() {
            continue;
        }
        // SAFETY: query_mode succeeded, so `info` points at a firmware-allocated GopModeInfo.
        let i = unsafe { &*info };
        if i.pixel_format == 3 || (i.horizontal_resolution as u64 * i.vertical_resolution as u64) > 1_100_000 || i.vertical_resolution > 768 {
            continue; // blt-only modes have no linear framebuffer; very large modes are slow to draw
        }
        let area = i.horizontal_resolution as u64 * i.vertical_resolution as u64;
        let better = match best {
            None => true,
            Some((bm, ba)) => area > ba || (area == ba && m == current && bm != current),
        };
        if better {
            best = Some((m, area));
        }
    }
    best.map(|(m, _)| m)
}

fn init_gop(bs: &BootServices) -> Option<FbInfo> {
    let mut p: *mut c_void = core::ptr::null_mut();
    if (bs.locate_protocol)(&uefi::GOP_GUID, core::ptr::null_mut(), &mut p) != uefi::SUCCESS || p.is_null() {
        return None;
    }
    let gop = p as *mut Gop;
    // SAFETY: locate_protocol returned a live Gop interface.
    let g = unsafe { &*gop };
    if g.mode.is_null() {
        return None;
    }
    if let Some(m) = pick_gop_mode(g) {
        // SAFETY: reading the current mode number from the firmware-owned mode record.
        if m != unsafe { (*g.mode).mode } {
            let _ = (g.set_mode)(gop, m);
        }
    }
    // SAFETY: `mode` and `info` stay valid after set_mode; re-read them since the mode changed.
    let (mode, i) = unsafe { (&*g.mode, &*(*g.mode).info) };
    let format = match i.pixel_format {
        0 => PixelFormat::Rgb,
        1 => PixelFormat::Bgr,
        2 => PixelFormat::Mask { r: i.red_mask, g: i.green_mask, b: i.blue_mask },
        _ => return None,
    };
    Some(FbInfo {
        base: mode.frame_buffer_base,
        size: mode.frame_buffer_size,
        width: i.horizontal_resolution as usize,
        height: i.vertical_resolution as usize,
        stride: i.pixels_per_scan_line as usize,
        format,
    })
}

fn read_var_u8(rt: &RuntimeServices, name: &[u16]) -> Option<u8> {
    let mut size = 1usize;
    let mut data = 0u8;
    let mut attr = 0u32;
    let s = (rt.get_variable)(name.as_ptr(), &uefi::GLOBAL_VARIABLE_GUID, &mut attr, &mut size, &mut data);
    (s == uefi::SUCCESS && size == 1).then_some(data)
}

fn secure_boot_state(rt: &RuntimeServices) -> SecureBoot {
    const SECURE_BOOT: [u16; 11] = uefi::utf16(b"SecureBoot\0");
    const SETUP_MODE: [u16; 10] = uefi::utf16(b"SetupMode\0");
    if read_var_u8(rt, &SETUP_MODE) == Some(1) {
        return SecureBoot::SetupMode;
    }
    match read_var_u8(rt, &SECURE_BOOT) {
        Some(1) => SecureBoot::Enabled,
        Some(0) => SecureBoot::Disabled,
        _ => SecureBoot::Unknown,
    }
}

/// True if the firmware exposes the TCG2 protocol (a TPM 2.0 is present and measured boot is possible).
fn tpm2_present(bs: &BootServices) -> bool {
    let mut p: *mut c_void = core::ptr::null_mut();
    (bs.locate_protocol)(&uefi::TCG2_GUID, core::ptr::null_mut(), &mut p) == uefi::SUCCESS && !p.is_null()
}

/// Builds the boot random seed from the firmware RNG and RDRAND (the strong sources) plus timing.
/// Returns the seed and how many strong sources contributed. The seed is never logged.
fn gather_seed(bs: &BootServices, rdrand: bool) -> ([u8; entropy::SEED_LEN], u8) {
    let mut pool = entropy::Pool::new();
    let mut strong = 0u8;

    let mut p: *mut c_void = core::ptr::null_mut();
    if (bs.locate_protocol)(&uefi::RNG_GUID, core::ptr::null_mut(), &mut p) == uefi::SUCCESS && !p.is_null() {
        let mut bytes = [0u8; 32];
        let rng = p as *mut Rng;
        // SAFETY: locate_protocol returned a live EFI_RNG_PROTOCOL; a null algorithm selects the default.
        let s = unsafe { ((*rng).get_rng)(rng, core::ptr::null(), bytes.len(), bytes.as_mut_ptr()) };
        if s == uefi::SUCCESS {
            pool.add(&bytes);
            strong += 1;
        } else {
            debug!("seed", "firmware RNG returned status={:#x}", s);
        }
        bytes.fill(0);
        core::hint::black_box(&bytes);
    }
    if rdrand {
        let words = [arch::rdrand64(), arch::rdrand64(), arch::rdrand64(), arch::rdrand64()];
        if words.iter().all(Option::is_some) {
            for w in words.into_iter().flatten() {
                pool.add_u64(w);
            }
            strong += 1;
        }
    }
    pool.add_u64(arch::rdtsc());
    pool.add_u64(boot_info().image_base);
    (pool.finish(), strong)
}

/// Reads one SHA-256 PCR through the TPM, or `None` if the TPM refuses or answers nonsense.
fn pcr_read(tcg: *mut Tcg2, pcr: u32) -> Option<[u8; 32]> {
    let mut cmd = [0u8; 20];
    let n = tcg2::build_pcr_read(&mut cmd, pcr);
    let mut resp = [0u8; 128];
    // SAFETY: `tcg` is a live EFI_TCG2_PROTOCOL; both blocks are valid for the sizes passed.
    let s = unsafe { ((*tcg).submit_command)(tcg, n as u32, cmd.as_ptr(), resp.len() as u32, resp.as_mut_ptr()) };
    if s != uefi::SUCCESS {
        debug!("tpm", "PCR_Read submit status={:#x}", s);
        return None;
    }
    tcg2::parse_pcr_read(&resp)
}

/// Measured boot: extends the boot command line into PCR 12 (and the firmware event log), then reads
/// the PCR back to prove the extend happened. The firmware has already measured this image itself.
fn measure_cmdline(bs: &BootServices, cmdline: &str) {
    let mut p: *mut c_void = core::ptr::null_mut();
    if (bs.locate_protocol)(&uefi::TCG2_GUID, core::ptr::null_mut(), &mut p) != uefi::SUCCESS || p.is_null() {
        info!("tpm", "measured boot unavailable: no TPM 2.0 (EFI_TCG2_PROTOCOL not found)");
        return;
    }
    let tcg = p as *mut Tcg2;

    let (mut loc, mut last, mut truncated) = (0u64, 0u64, 0u8);
    // SAFETY: live protocol; format 2 is the crypto-agile log; the three out-pointers are valid.
    let s = unsafe { ((*tcg).get_event_log)(tcg, 2, &mut loc, &mut last, &mut truncated) };
    if s == uefi::SUCCESS {
        info!("tpm", "event log at {:#x}, last entry {:#x}, truncated={}", loc, last, truncated != 0);
    } else {
        warn!("tpm", "GetEventLog status={:#x}", s);
    }

    let before = pcr_read(tcg, tcg2::PCR_CMDLINE);
    let mut data = [0u8; 2 * 130];
    let Some(dlen) = tcg2::utf16le_cmdline(cmdline, &mut data) else {
        warn!("tpm", "command line too long to measure");
        return;
    };
    let mut ev = [0u8; tcg2::EVENT_OVERHEAD + 2 * 130];
    let Some(elen) = tcg2::build_event(&mut ev, tcg2::PCR_CMDLINE, tcg2::EV_IPL, &data[..dlen]) else {
        return;
    };
    let _ = elen;
    // SAFETY: live protocol; `data` and `ev` stay valid for the call and `ev` is a well-formed
    // EFI_TCG2_EVENT built by `tcg2::build_event`.
    let s = unsafe { ((*tcg).hash_log_extend_event)(tcg, 0, data.as_ptr() as u64, dlen as u64, ev.as_ptr()) };
    if s != uefi::SUCCESS {
        warn!("tpm", "HashLogExtendEvent status={:#x}", s);
        return;
    }
    let after = pcr_read(tcg, tcg2::PCR_CMDLINE);
    let mut hex = [0u8; 64];
    if let (Some(b), Some(a)) = (before, after) {
        let verified = tcg2::expected_extend(&b, &data[..dlen]) == a;
        tcg2::hex32(&b, &mut hex);
        info!("tpm", "pcr{}_before={}", tcg2::PCR_CMDLINE, core::str::from_utf8(&hex).unwrap_or("?"));
        tcg2::hex32(&a, &mut hex);
        info!("tpm", "pcr{}_after={}", tcg2::PCR_CMDLINE, core::str::from_utf8(&hex).unwrap_or("?"));
        info!("tpm", "measured cmdline into pcr={} event=EV_IPL bytes={} verified={}", tcg2::PCR_CMDLINE, dlen, verified);
        if !verified {
            error!("tpm", "PCR {} does not hold the expected value after the extend", tcg2::PCR_CMDLINE);
        }
    } else {
        warn!("tpm", "measured cmdline into pcr={} but could not read the PCR back", tcg2::PCR_CMDLINE);
    }
}

/// Image base/size and the command line (UEFI load options), copied into `BootInfo`.
fn read_loaded_image(bs: &BootServices, image: Handle) {
    let b = boot_info();
    let mut li: *mut c_void = core::ptr::null_mut();
    if (bs.handle_protocol)(image, &uefi::LOADED_IMAGE_GUID, &mut li) == uefi::SUCCESS && !li.is_null() {
        // SAFETY: handle_protocol returned the LoadedImage of our own image handle.
        let li = unsafe { &*(li as *const LoadedImage) };
        b.image_base = li.image_base as u64;
        b.image_size = li.image_size;
        let units = (li.load_options_size as usize / 2).min(b.cmdline.len() - 1);
        if !li.load_options.is_null() {
            let opts = li.load_options as *const u16;
            for i in 0..units {
                // SAFETY: `units` 16-bit words lie within load_options_size bytes.
                let c = unsafe { opts.add(i).read_unaligned() };
                b.cmdline[i] = if (0x20..0x7F).contains(&c) { c as u8 } else { b'?' };
            }
            b.cmdline_len = units;
        }
    }
}

fn find_acpi(st: &SystemTable) -> (u64, Rsdp) {
    let guids: [&Guid; 2] = [&uefi::ACPI_20_GUID, &uefi::ACPI_10_GUID];
    for g in guids {
        let Some(p) = st.config_table(g) else { continue };
        let p = p as *const u8;
        // SAFETY: an RSDP is at least 20 bytes and firmware allocates the ACPI 2.0 structure (36
        // bytes) whole; we first read only the 24-byte header to learn how much more is valid.
        let head = unsafe { core::slice::from_raw_parts(p, 24) };
        let Some((rev, len)) = acpi::peek_header(head) else {
            return (p as u64, Rsdp::Invalid);
        };
        let n = if rev == 0 { 24 } else { len.clamp(24, acpi::V2_MAX_LEN) };
        // SAFETY: `n` is the table length declared by the table itself, capped at V2_MAX_LEN; a
        // lying length is caught by the checksum, and the read stays within one firmware table.
        let all = unsafe { core::slice::from_raw_parts(p, n) };
        return (p as u64, acpi::validate(all));
    }
    (0, Rsdp::Invalid)
}

/// Fetches the memory map into `buf`. On `BUFFER_TOO_SMALL` the needed size is returned in `Err.1`.
fn fetch_map(bs: &BootServices, buf: *mut u8, cap: usize) -> Result<(usize, usize, usize), (Status, usize)> {
    let (mut size, mut key, mut dsz, mut dver) = (cap, 0usize, 0usize, 0u32);
    let s = (bs.get_memory_map)(&mut size, buf, &mut key, &mut dsz, &mut dver);
    if s == uefi::SUCCESS {
        Ok((size, key, dsz))
    } else {
        Err((s, size))
    }
}

fn alloc_pool(bs: &BootServices, size: usize) -> Option<*mut u8> {
    let mut p: *mut u8 = core::ptr::null_mut();
    ((bs.allocate_pool)(uefi::MEM_LOADER_DATA, size, &mut p) == uefi::SUCCESS && !p.is_null()).then_some(p)
}

/// Publishes an ASCII string as a volatile NUL-terminated UTF-16 variable under the systemd Boot
/// Loader Interface GUID, so Linux tools such as `bootctl` can see which loader started the system.
fn set_loader_var(rt: &RuntimeServices, name: &[u16], value: &str) {
    let mut buf = [0u16; 48];
    let n = value.len().min(buf.len() - 1);
    for (i, b) in value.bytes().take(n).enumerate() {
        buf[i] = b as u16;
    }
    let _ = (rt.set_variable)(name.as_ptr(), &uefi::LOADER_GUID, uefi::VAR_BS_RT, (n + 1) * 2, buf.as_ptr().cast());
}

/// Next key waiting in the firmware's input queue, as `(scan_code, unicode)`.
fn read_key(st: &SystemTable) -> Option<(u16, u16)> {
    if st.con_in.is_null() {
        return None;
    }
    let mut k = InputKey { scan_code: 0, unicode_char: 0 };
    // SAFETY: con_in is the firmware's text-input protocol, valid until ExitBootServices.
    let s = unsafe { ((*st.con_in).read_key_stroke)(st.con_in, &mut k) };
    if s != uefi::SUCCESS && s != uefi::NOT_READY {
        debug!("menu", "read_key_stroke status={:#x}", s);
    }
    (s == uefi::SUCCESS).then_some((k.scan_code, k.unicode_char))
}

fn read_var_u64(rt: &RuntimeServices, name: &[u16]) -> Option<u64> {
    let mut size = 8usize;
    let mut data = [0u8; 8];
    let mut attr = 0u32;
    let s = (rt.get_variable)(name.as_ptr(), &uefi::GLOBAL_VARIABLE_GUID, &mut attr, &mut size, data.as_mut_ptr());
    (s == uefi::SUCCESS && size <= 8).then(|| u64::from_le_bytes(data))
}

/// Asks the firmware to open its setup UI on the next boot, then resets. This is the OsIndications
/// mechanism behind `systemctl reboot --firmware-setup` and Windows' "UEFI Firmware Settings".
fn reboot_to_firmware_setup(rt: &RuntimeServices) {
    const SUPPORTED: [u16; 23] = uefi::utf16(b"OsIndicationsSupported\0");
    const INDICATIONS: [u16; 14] = uefi::utf16(b"OsIndications\0");
    const BOOT_TO_FW_UI: u64 = 1;
    if read_var_u64(rt, &SUPPORTED).unwrap_or(0) & BOOT_TO_FW_UI == 0 {
        warn!("menu", "firmware does not support booting to its setup UI");
        return;
    }
    let value = read_var_u64(rt, &INDICATIONS).unwrap_or(0) | BOOT_TO_FW_UI;
    let s = (rt.set_variable)(INDICATIONS.as_ptr(), &uefi::GLOBAL_VARIABLE_GUID, uefi::VAR_NV_BS_RT, 8, value.to_le_bytes().as_ptr());
    if s != uefi::SUCCESS {
        warn!("menu", "could not set OsIndications, status={:#x}", s);
        return;
    }
    info!("menu", "rebooting into firmware setup");
    (rt.reset_system)(uefi::RESET_COLD, 0, 0, core::ptr::null());
}

/// Boot menu in the spirit of GRUB, systemd-boot and the Windows Boot Manager: wait a few seconds for
/// a key (through the firmware, so USB keyboards work), otherwise continue booting. Returns the
/// possibly updated options.
fn boot_menu(st: &SystemTable, bs: &BootServices, rt: &RuntimeServices, mut opts: BootOpts) -> BootOpts {
    if opts.menu_timeout == 0 || st.con_in.is_null() {
        return opts;
    }
    // SAFETY: con_in is a live protocol (checked non-null); resetting also drops stale keystrokes.
    unsafe { ((*st.con_in).reset)(st.con_in, false) };
    info!("menu", "press any key within {} s for the boot menu", opts.menu_timeout);
    let _ = log::with_console(|c| c.hint(b"press any key for the boot menu"));
    let mut pressed = false;
    let t0 = aeon::arch::rdtsc();
    let budget = log::tsc_hz().saturating_mul(u64::from(opts.menu_timeout));
    while aeon::arch::rdtsc().wrapping_sub(t0) < budget {
        let _ = (bs.stall)(20_000);
        if read_key(st).is_some() {
            pressed = true;
            break;
        }
    }
    let _ = log::with_console(|c| c.hint(b""));
    if !pressed {
        return opts;
    }
    // The menu is text: leave the splash and show everything logged so far.
    if log::with_console(|c| {
        let was = c.splash_active();
        c.end_splash();
        was
    })
    .unwrap_or(false)
    {
        log::replay(|bytes| {
            let _ = log::with_console(|c| c.write_line(Level::Info, bytes));
        });
    }
    loop {
        info!("menu", "Enter boot | D toggle debug logging | F firmware setup | R reboot | S shut down");
        let (scan, ch) = loop {
            let _ = (bs.stall)(20_000);
            if let Some(k) = read_key(st) {
                break k;
            }
        };
        match (scan, ch as u8 as char) {
            (_, '\r') | (_, ' ') => return opts,
            (_, 'd' | 'D') => {
                opts.debug = !opts.debug;
                opts.quiet = false;
                log::set_max_level(if opts.debug { Level::Trace } else { Level::Debug });
                info!("menu", "debug logging {}", if opts.debug { "on" } else { "off" });
            }
            (_, 'f' | 'F') => reboot_to_firmware_setup(rt),
            (_, 'r' | 'R') => (rt.reset_system)(uefi::RESET_COLD, 0, 0, core::ptr::null()),
            (_, 's' | 'S') => (rt.reset_system)(uefi::RESET_SHUTDOWN, 0, 0, core::ptr::null()),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------------------------

// The firmware entry point cannot be `unsafe`; the firmware guarantees the system-table pointer.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "efiapi" fn efi_main(image: Handle, st_ptr: *mut SystemTable) -> Status {
    if st_ptr.is_null() {
        return uefi::INVALID_PARAMETER;
    }
    // SAFETY: the firmware passes a valid system table to the image entry point.
    let st = unsafe { &*st_ptr };
    if st.boot_services.is_null() || st.runtime_services.is_null() {
        return uefi::INVALID_PARAMETER;
    }
    // SAFETY: checked non-null above; boot services stay valid until ExitBootServices below.
    let bs = unsafe { &*st.boot_services };
    // SAFETY: checked non-null above; runtime services stay valid after ExitBootServices.
    let rt = unsafe { &*st.runtime_services };

    // ---- Stage 1: firmware entry ----
    log::init();
    // Stop the 5-minute firmware watchdog, which would otherwise reset the machine while we run.
    let wd = (bs.set_watchdog_timer)(0, 0, 0, core::ptr::null());
    let hz = calibrate_tsc(bs);
    let hz_ok = hz != 0;
    log::set_tsc_hz(if hz_ok { hz } else { 2_000_000_000 });
    let serial = log::enable_serial();
    let debugcon = log::enable_debugcon();
    // The firmware console is mirrored to the serial port by many firmwares, so use it as a log
    // sink only when there is no UART to read the log from.
    if !serial {
        log::attach_conout(st.con_out);
    }
    stage("firmware-entry");
    info!(
        "boot",
        "Aeon {} git={} arch=x86_64 profile={}",
        aeon::VERSION,
        aeon::GIT_HASH,
        if cfg!(debug_assertions) { "dev" } else { "release" }
    );
    info!("boot", "sinks serial={} debugcon={} watchdog_disabled={}", serial, debugcon, wd == uefi::SUCCESS);
    if hz_ok {
        info!("time", "tsc_hz={} source=uefi-stall", hz);
    } else {
        warn!("time", "tsc calibration implausible, assuming 2.0 GHz; timestamps are approximate");
    }
    if !serial {
        warn!("serial", "COM1 not present (scratch-register test failed); using screen and debugcon only");
    }

    // ---- Stage 2: early init ----
    stage("early-init");
    let leaves = arch::read_leaves();
    let feat = cpuid::decode(&leaves);
    let vendor = arch::vendor();
    info!(
        "cpu",
        "vendor={} max_leaf={:#x} max_ext={:#x} phys_bits={} virt_bits={}",
        core::str::from_utf8(&vendor).unwrap_or("?"),
        leaves.max_basic,
        leaves.max_ext,
        feat.phys_bits,
        feat.virt_bits
    );
    info!(
        "cpu",
        "supported nx={} smep={} smap={} umip={} sse2={} rdrand={} hypervisor={}",
        feat.nx,
        feat.smep,
        feat.smap,
        feat.umip,
        feat.sse2,
        feat.rdrand,
        feat.hypervisor
    );
    let s = arch::cpu_state();
    info!(
        "cpu",
        "state-at-entry cr0.wp={} cr4.smep={} cr4.smap={} cr4.umip={} efer.nxe={}",
        s.cr0 & arch::CR0_WP != 0,
        s.cr4 & arch::CR4_SMEP != 0,
        s.cr4 & arch::CR4_SMAP != 0,
        s.cr4 & arch::CR4_UMIP != 0,
        s.efer & arch::EFER_NXE != 0
    );

    // Image info and boot options come first: they decide whether the screen shows the splash or the log.
    read_loaded_image(bs, image);
    let mut opts = bootopts::parse(boot_info().cmdline_str());
    if opts.quiet {
        log::set_max_level(Level::Warn);
    } else if opts.debug {
        log::set_max_level(Level::Trace);
    }

    let fb = init_gop(bs);
    if let Some(f) = fb {
        // SAFETY: GOP reported this linear framebuffer; it is mapped by the firmware and stays
        // valid across ExitBootServices, and nothing else draws to it after this point.
        if let Some(mut c) = unsafe { Console::new(&f) } {
            if opts.verbose {
                c.clear();
                c.draw_logo();
            } else {
                // Splash: the logo fades in, a thin bar tracks the real boot stages, the log is held back.
                c.begin_splash();
                for step in 1..=10u32 {
                    c.fade_logo((step * 255 / 10) as u8);
                    let _ = (bs.stall)(18_000);
                }
            }
            let (cols, rows) = c.size();
            log::detach_conout();
            log::attach_console(c);
            // The firmware console no longer shares the screen; hide its cursor.
            if !st.con_out.is_null() {
                // SAFETY: con_out is a live text-output protocol until ExitBootServices.
                unsafe { ((*st.con_out).enable_cursor)(st.con_out, false) };
            }
            info!("fb", "gop {}x{} stride={} fmt={:?} base={:#x} console={}x{}", f.width, f.height, f.stride, f.format, f.base, cols, rows);
        } else {
            warn!("fb", "framebuffer too small for console, using firmware console");
        }
    } else {
        warn!("fb", "no usable GOP framebuffer, using firmware console");
    }
    {
        let b = boot_info();
        b.fb = fb;
        b.tsc_hz = log::tsc_hz();
    }
    {
        // Catch the bar up with the stages that ran before the console existed.
        // SAFETY: single-threaded boot; plain read of the stage counter.
        let done = unsafe { (*STAGES.as_ptr()).n }.saturating_sub(1);
        let _ = log::with_console(|c| c.progress(splash::stage_permille(done, TOTAL_STAGES)));
    }

    // ---- Stage 3: platform discovery ----
    stage("platform-discovery");
    {
        let b = boot_info();
        b.fw_vendor_len = {
            // SAFETY: firmware_vendor is a NUL-terminated UTF-16 string supplied by the firmware.
            unsafe { uefi::utf16_to_ascii(st.firmware_vendor, &mut b.fw_vendor) }
        };
        b.fw_revision = st.firmware_revision;
        b.uefi_revision = st.hdr.revision;
        b.runtime_services = st.runtime_services as usize;

        let (rsdp, parsed) = find_acpi(st);
        b.acpi_rsdp = rsdp;
        b.acpi = parsed;
        b.smbios = st
            .config_table(&uefi::SMBIOS3_GUID)
            .or_else(|| st.config_table(&uefi::SMBIOS_GUID))
            .map_or(0, |p| p as u64);
        b.secure_boot = secure_boot_state(rt);
    }
    {
        let b = boot_info();
        let rev = b.uefi_revision;
        info!(
            "fw",
            "vendor=\"{}\" fw_rev={:#x} uefi={}.{}.{}",
            b.fw_vendor_str(),
            b.fw_revision,
            rev >> 16,
            (rev & 0xFFFF) / 10,
            (rev & 0xFFFF) % 10
        );
        info!("fw", "secure_boot={} tpm2={}", b.secure_boot.name(), tpm2_present(bs));
        info!(
            "image",
            "base={:#x} size={:#x} cmdline=\"{}\"",
            b.image_base,
            b.image_size,
            b.cmdline_str()
        );
        match b.acpi {
            Rsdp::V2 { xsdt, revision } => {
                info!("acpi", "rsdp={:#x} rev={} xsdt={:#x} checksum=ok", b.acpi_rsdp, revision, xsdt)
            }
            Rsdp::V1 { rsdt } => info!("acpi", "rsdp={:#x} rev=0 rsdt={:#x} checksum=ok", b.acpi_rsdp, rsdt),
            Rsdp::Invalid if b.acpi_rsdp == 0 => warn!("acpi", "no RSDP in configuration table"),
            Rsdp::Invalid => warn!("acpi", "rsdp={:#x} failed validation, ignoring ACPI", b.acpi_rsdp),
        }
        if b.smbios != 0 {
            info!("smbios", "entry={:#x}", b.smbios);
        } else {
            debug!("smbios", "not present");
        }
    }

    // ---- Stage 4: security init (random seed, measured boot) ----
    stage("security-init");
    {
        let (seed, strong) = gather_seed(bs, feat.rdrand);
        let b = boot_info();
        b.seed = seed;
        b.seed_sources = strong;
        sched::set_canary(entropy::canary_from(&seed));
        if strong == 0 {
            warn!("seed", "no hardware entropy source (firmware RNG, RDRAND): seed rests on timing only");
        } else {
            info!("seed", "boot seed ready from {} hardware source(s); stack canary randomised", strong);
        }
    }
    measure_cmdline(bs, boot_info().cmdline_str());

    // Boot menu (through the firmware console, so USB keyboards work here).
    opts = boot_menu(st, bs, rt, opts);
    let _ = opts;
    set_loader_var(rt, &uefi::utf16(b"LoaderInfo\0"), concat!("Aeon ", env!("CARGO_PKG_VERSION")));

    // ---- Stage 5: ExitBootServices ----
    stage("exit-boot-services");
    // Size the buffer from the firmware's own answer, with slack because the allocation below (and
    // the first failed ExitBootServices) can add descriptors.
    let probe = fetch_map(bs, core::ptr::null_mut(), 0);
    let (need, desc_hint) = match probe {
        Err((uefi::BUFFER_TOO_SMALL, need)) => (need, 48usize),
        _ => (16 * 1024, 48usize),
    };
    let mut cap = need + 16 * desc_hint;
    let Some(mut buf) = alloc_pool(bs, cap) else {
        error!("mem", "cannot allocate {} bytes for the memory map", cap);
        return uefi::BUFFER_TOO_SMALL;
    };
    log::detach_conout();

    let mut attempts = 0;
    let (map_size, desc_size) = loop {
        let (size, key, dsz) = match fetch_map(bs, buf, cap) {
            Ok(v) => v,
            Err((uefi::BUFFER_TOO_SMALL, need)) if attempts < 4 => {
                cap = need + 16 * desc_hint;
                match alloc_pool(bs, cap) {
                    Some(p) => buf = p,
                    None => return uefi::BUFFER_TOO_SMALL,
                }
                attempts += 1;
                continue;
            }
            Err((s, _)) => {
                error!("mem", "GetMemoryMap failed status={:#x}", s);
                return s;
            }
        };
        // No logging and no allocation between GetMemoryMap and ExitBootServices: either could
        // change the map and invalidate `key`.
        let s = (bs.exit_boot_services)(image, key);
        if s == uefi::SUCCESS {
            break (size, dsz);
        }
        attempts += 1;
        if attempts > 4 {
            error!("boot", "ExitBootServices failed status={:#x} after {} attempts", s, attempts);
            return s;
        }
    };
    arch::disable_interrupts();

    // ---- Boot services are gone: from here on only our own code and runtime services ----
    {
        let b = boot_info();
        // SAFETY: `buf` holds `map_size` bytes written by GetMemoryMap; the pool allocation stays
        // valid because nothing frees it and the loader-data region is mapped by `paging::build`.
        let raw = unsafe { core::slice::from_raw_parts(buf, map_size) };
        let (n, dropped) = memmap::collect_regions(raw, desc_size, &mut b.regions);
        b.region_count = n;
        b.regions_dropped = dropped;
        b.mem = memmap::summarize(b.regions());
    }
    stage("kernel-env");
    info!("boot", "boot services exited after {} attempt(s)", attempts + 1);
    {
        let b = boot_info();
        info!(
            "mem",
            "descriptors={} kept={} dropped={} desc_size={}",
            map_size / desc_size,
            b.region_count,
            b.regions_dropped,
            desc_size
        );
        if b.regions_dropped > 0 {
            warn!("mem", "memory map has more than {} merged regions; {} were not recorded", MAX_REGIONS, b.regions_dropped);
        }
        let m = &b.mem;
        info!(
            "mem",
            "ram_mb={} usable_mb={} reclaimable_mb={} acpi_mb={} runtime_mb={} mmio_mb={} top_of_ram_mb={}",
            memmap::pages_to_mb(m.ram_pages),
            memmap::pages_to_mb(m.usable_pages),
            memmap::pages_to_mb(m.reclaimable_pages),
            memmap::pages_to_mb(m.acpi_pages),
            memmap::pages_to_mb(m.runtime_pages),
            memmap::pages_to_mb(m.mmio_pages),
            m.top_of_ram / (1024 * 1024)
        );
        info!("mem", "largest_free_block_mb={}", memmap::pages_to_mb(m.largest_usable_pages));
    }

    // Own execution environment. The order matters: NX must be enabled before page tables with NX
    // bits are loaded, and the IDT must exist before anything can fault.
    // SAFETY: single CPU, interrupts disabled, boot services exited; each step's own contract is
    // met by the arguments below.
    unsafe {
        let nx = feat.nx;
        if nx {
            arch::enable_nxe();
        }
        let wc = arch::has_pat();
        if wc {
            arch::program_pat_wc();
        }
        let stack = KSTACK.as_ptr() as u64;
        let stack_top = stack + KSTACK_SIZE as u64;
        arch::init_descriptors(stack_top);
        let stats = paging::build(boot_info(), nx, wc, stack);
        paging::activate();
        info!(
            "paging",
            "identity map active: pd_tables={} split_tables={} skipped={} nx={} wc={}",
            stats.pd_used,
            stats.pt_used,
            stats.skipped,
            nx,
            wc
        );
        let after = arch::enable_protections(&feat);
        info!(
            "cpu",
            "state-now cr0.wp={} cr4.smep={} cr4.smap={} cr4.umip={} efer.nxe={}",
            after.cr0 & arch::CR0_WP != 0,
            after.cr4 & arch::CR4_SMEP != 0,
            after.cr4 & arch::CR4_SMAP != 0,
            after.cr4 & arch::CR4_UMIP != 0,
            after.efer & arch::EFER_NXE != 0
        );
        info!("cpu", "gdt/tss/idt loaded, double-fault IST stack set, switching to kernel stack {:#x}..{:#x}", stack, stack_top);
        arch::call_on_stack(stack_top, kernel_main)
    }
}

// ---------------------------------------------------------------------------------------------
// Kernel
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "fault-stack")]
#[allow(unconditional_recursion)]
#[inline(never)]
fn overflow_stack(depth: u64) -> u64 {
    let pad = [depth as u8; 512];
    core::hint::black_box(&pad);
    overflow_stack(depth + 1) + pad[0] as u64
}

fn run_fault_tests() {
    #[cfg(feature = "fault-ud")]
    {
        warn!("test", "fault-ud: executing ud2");
        // SAFETY: deliberately raises #UD to exercise the exception path.
        unsafe { core::arch::asm!("ud2") };
    }
    #[cfg(feature = "fault-pf")]
    {
        warn!("test", "fault-pf: reading address 0");
        // SAFETY: deliberately faults; page 0 is unmapped.
        let v = unsafe { core::ptr::read_volatile(core::hint::black_box(core::ptr::null::<u8>())) };
        info!("test", "unexpectedly read {}", v);
    }
    #[cfg(feature = "fault-stack")]
    {
        warn!("test", "fault-stack: recursing until the guard page");
        let v = overflow_stack(0);
        info!("test", "unexpected return {}", v);
    }
    #[cfg(feature = "fault-wx")]
    {
        warn!("test", "fault-wx: writing to kernel code");
        let code = run_fault_tests as *const () as *mut u8;
        // SAFETY: deliberately faults; code pages are mapped read-only and CR0.WP is set.
        unsafe { code.write_volatile(0x90) };
        info!("test", "unexpected: code page was writable");
    }
    #[cfg(feature = "fault-nx")]
    {
        warn!("test", "fault-nx: executing from the stack");
        let mut buf = [0xC3u8; 16]; // `ret`
        // SAFETY: deliberately jumps into non-executable memory to provoke a #PF (instruction fetch).
        let f: extern "C" fn() = unsafe { core::mem::transmute(core::hint::black_box(buf.as_mut_ptr())) };
        f();
        info!("test", "unexpected: stack was executable");
    }
    #[cfg(feature = "fault-panic")]
    {
        warn!("test", "fault-panic: panicking");
        panic!("deliberate panic (fault-panic feature)");
    }
}

extern "C" fn kernel_main() -> ! {
    let rsp: u64;
    // SAFETY: reads the stack pointer register only.
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags)) };
    info!("kernel", "kernel_main entered on kernel stack rsp={:#x}", rsp);

    stage("kernel-memory");
    // Memory management: physical frames first, then the heap carved from them.
    // SAFETY: boot services have exited, the map in BootInfo is final, and this runs once.
    let fs = unsafe { mm::init_frames(boot_info()) };
    info!(
        "mm",
        "frames: {} free ({} MB), largest run {} MB",
        fs.free,
        fs.free as u64 * 4 / 1024,
        fs.largest_run as u64 * 4 / 1024
    );
    // SAFETY: init_frames has run; this is the only heap initialisation.
    let heap = unsafe { mm::init_heap() };
    if heap == 0 {
        error!("mm", "could not allocate a kernel heap");
    } else {
        info!("mm", "heap: {} KiB", heap / 1024);
    }

    // Tick source: PIC + PIT. If no interrupt arrives, fall back to polling instead of hanging.
    // SAFETY: the IDT routes vector 32 to the tick stub (set up by `init_descriptors`).
    unsafe { arch::start_timer() };
    arch::enable_interrupts();
    let t0 = arch::rdtsc();
    let limit = log::tsc_hz() / 3; // 330 ms
    while arch::ticks() < 3 && arch::rdtsc().wrapping_sub(t0) < limit {
        arch::pause();
    }
    let irq_mode = arch::ticks() >= 3;
    if irq_mode {
        info!("timer", "pit irq0 ok hz={} ticks_seen={}", arch::TIMER_HZ, arch::ticks());
    } else {
        arch::disable_interrupts();
        warn!("timer", "no timer interrupt within 330 ms; no preemption, running the shell directly");
    }

    stage("kernel-tasks");
    // SAFETY: reads the stage table written during boot; no writer is active.
    let stages = unsafe { &*STAGES.as_ptr() };
    let hz = log::tsc_hz();
    for w in stages.items[..stages.n].windows(2) {
        info!("stage", "{} took {} us", w[0].0, logfmt::ticks_to_us(w[1].1 - w[0].1, hz));
    }
    if stages.n > 0 {
        let last = stages.items[stages.n - 1];
        info!("stage", "{} took {} us", last.0, logfmt::ticks_to_us(arch::rdtsc() - last.1, hz));
    }
    info!("boot", "boot complete uptime_us={} mode={}", log::uptime_us(), if irq_mode { "preemptive" } else { "single-task" });

    // Finish the splash: bar to 100 %, a short hold so the eye sees it complete, then the shell screen.
    if log::with_console(|c| c.splash_active()).unwrap_or(false) {
        let _ = log::with_console(|c| c.progress(1000));
        let t0 = arch::rdtsc();
        while arch::rdtsc().wrapping_sub(t0) < log::tsc_hz() / 5 {
            arch::pause();
        }
        let _ = log::with_console(|c| c.end_splash());
    }

    run_fault_tests();

    if !irq_mode {
        shell::shell_task(0);
        arch::halt_forever();
    }
    sched::init();
    for (name, entry) in [("status", shell::status_task as extern "C" fn(usize)), ("shell", shell::shell_task)] {
        match sched::spawn(name, entry, 0) {
            Some(id) => info!("sched", "task {} started as id {}", name, id),
            None => error!("sched", "cannot start task {}", name),
        }
    }
    // The boot thread becomes the idle task: it runs only when nothing else is ready.
    loop {
        arch::wait_for_interrupt();
    }
}

// ---------------------------------------------------------------------------------------------
// Panic
// ---------------------------------------------------------------------------------------------

static PANICKING: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if PANICKING.swap(true, Ordering::SeqCst) {
        log::raw_panic_line("[AEON] nested panic, halting\n");
        arch::halt_forever();
    }
    // The panic may have happened inside the logger; release it and show every level.
    log::reset_after_fault();
    log::set_max_level(Level::Trace);
    match info.location() {
        Some(l) => error!("panic", "panicked at {}:{}:{}", l.file(), l.line(), l.column()),
        None => error!("panic", "panicked at unknown location"),
    }
    error!("panic", "{}", info.message());
    error!("panic", "cr2={:#x} cr3={:#x} system halted", arch::read_cr2(), arch::read_cr3());
    // The screen gets where and why, never addresses or registers (those stay on serial/debugcon).
    let mut where_ = logfmt::LineBuf::<96>::new();
    let mut why = logfmt::LineBuf::<160>::new();
    match info.location() {
        Some(l) => {
            let file = l.file().rsplit(['/', '\\']).next().unwrap_or("?");
            let _ = write!(where_, "panicked at {}:{}", file, l.line());
        }
        None => {
            let _ = write!(where_, "panicked at an unknown location");
        }
    }
    let _ = write!(why, "{}", info.message());
    let (where_line, why_line) = (where_.finish().trim_ascii_end(), why.finish().trim_ascii_end());
    let _ = log::with_console(|c| c.panic_screen(&[where_line, why_line, b"Details are in the serial log."]));
    arch::halt_forever()
}
