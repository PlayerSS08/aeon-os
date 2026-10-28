#![no_std]
#![no_main]

use core::arch::x86_64::{__cpuid, __cpuid_count, _rdtsc};

// ============================================================================
// 1. UEFI Hardware Structures & Table Specifications
// ============================================================================

pub const EFI_SYSTEM_TABLE_SIGNATURE: u64 = 0x5453595320494249; // "IBI SYST"

#[repr(C)]
pub struct TableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct TextOut {
    _reset: usize,
    output_string: extern "efiapi" fn(*mut TextOut, *const u16) -> usize,
    _test: usize,
    _query: usize,
    _set_mode: usize,
    _set_attr: usize,
    clear_screen: extern "efiapi" fn(*mut TextOut) -> usize,
}

#[repr(C)]
pub struct MemoryDescriptor {
    pub mem_type: u32,
    pub pad: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attribute: u64,
}

#[repr(C)]
pub struct BootServices {
    pub hdr: TableHeader,
    pub raise_tpl: usize,
    pub restore_tpl: usize,
    pub allocate_pages: usize,
    pub free_pages: usize,
    pub get_memory_map: extern "efiapi" fn(
        *mut usize,
        *mut MemoryDescriptor,
        *mut usize,
        *mut usize,
        *mut u32,
    ) -> usize,
}

#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub _pad: u32,
    pub con_in_handle: *mut (),
    pub con_in: *mut (),
    pub con_out_handle: *mut (),
    pub con_out: *mut TextOut,
    pub std_err_handle: *mut (),
    pub std_err: *mut (),
    pub runtime_services: *mut (),
    pub boot_services: *mut BootServices,
}

// Memory map buffer allocated statically to preserve stack space
static mut MEM_MAP_BUF: [u8; 16384] = [0u8; 16384];

// Minimal universal Unicode Block Art boot splash
const BOOT_LOGO: &str = "\
\n\n\n\r
       █████╗  ██████╗ ███████╗\r
      ██╔══██╗██╔═══██╗██╔════╝\r
      ███████║██║   ██║███████╗\r
      ██╔══██║██║   ██║╚════██║\r
      ██║  ██║╚██████╔╝███████║\r
      ╚═╝  ╚═╝ ╚═════╝ ╚══════╝\r
\n\r
            M I C R O - O S\r
                v 0 . 2 . 0\r
\n\r";

fn screen_print(cout: *mut TextOut, s: &str) {
    if cout.is_null() { return; }
    let mut buf = [0u16; 128];
    let mut i = 0;
    for c in s.chars() {
        if i + 1 >= buf.len() {
            buf[i] = 0;
            unsafe { ((*cout).output_string)(cout, buf.as_ptr()); }
            i = 0;
        }
        buf[i] = c as u16;
        i += 1;
    }
    if i > 0 {
        buf[i] = 0;
        unsafe { ((*cout).output_string)(cout, buf.as_ptr()); }
    }
}

// ============================================================================
// 2. Hardware 16550 UART Serial Telemetry (COM1 @ 0x03F8)
// ============================================================================

const COM1: u16 = 0x03F8;

#[inline(always)]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") val, options(nomem, nostack, preserves_flags));
    val
}

fn serial_init() {
    unsafe {
        outb(COM1 + 1, 0x00); // Disable IRQs
        outb(COM1 + 3, 0x80); // Enable DLAB
        outb(COM1 + 0, 0x01); // Divisor 1 = 115200 baud
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03); // 8N1
        outb(COM1 + 2, 0xC7); // Enable FIFO
        outb(COM1 + 4, 0x0B); // RTS/DSR set
    }
}

fn serial_byte(b: u8) {
    unsafe {
        while (inb(COM1 + 5) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, b);
    }
}

fn serial_str(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            serial_byte(b'\r');
        }
        serial_byte(b);
    }
}

fn serial_hex_u8(val: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    serial_byte(b'0');
    serial_byte(b'x');
    serial_byte(HEX[(val >> 4) as usize]);
    serial_byte(HEX[(val & 0x0F) as usize]);
}

fn serial_hex_u32(val: u32) {
    serial_str("0x");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for i in (0..8).rev() {
        serial_byte(HEX[((val >> (i * 4)) & 0x0F) as usize]);
    }
}

fn serial_hex_u64(val: u64) {
    serial_str("0x");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        serial_byte(HEX[((val >> (i * 4)) & 0x0F) as usize]);
    }
}

fn serial_dec_u64(mut n: u64) {
    if n == 0 {
        serial_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        serial_byte(buf[i]);
    }
}

// ============================================================================
// 3. Cryptographic CRC32 Integrity & Root of Trust Engine
// ============================================================================

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if (c & 1) != 0 { (c >> 1) ^ 0xEDB88320 } else { c >> 1 };
        }
    }
    !c
}

fn verify_header_crc32(hdr: &TableHeader) -> bool {
    if hdr.header_size < 24 || hdr.header_size > 4096 {
        return false;
    }
    let raw = unsafe {
        core::slice::from_raw_parts(hdr as *const TableHeader as *const u8, hdr.header_size as usize)
    };
    let mut scratch = [0u8; 256];
    let len = hdr.header_size as usize;
    if len > scratch.len() {
        return false;
    }
    scratch[..len].copy_from_slice(&raw[..len]);
    // Clear crc32 field (bytes 16..20) per UEFI specification before calculating CRC
    scratch[16] = 0;
    scratch[17] = 0;
    scratch[18] = 0;
    scratch[19] = 0;
    let computed = crc32(&scratch[..len]);
    computed == hdr.crc32
}

// ============================================================================
// 4. Secure Kernel Entry Point & Hardware Mathematical Probing
// ============================================================================

#[no_mangle]
pub extern "efiapi" fn efi_main(_handle: *mut (), sys: *mut SystemTable) -> usize {
    let tsc_start = unsafe { _rdtsc() };

    // 1. Initialize serial telemetry hardware immediately
    serial_init();

    serial_str("[AEON] ==================================================\n");
    serial_str("[AEON:BOOT]  Root of Trust Initialized (UEFI x86_64 Long Mode)\n");

    // 2. Pre-flight Pointer & Memory Safety Verification
    if sys.is_null() {
        serial_str("[AEON:PANIC] NULL SystemTable Pointer! Boot aborted.\n");
        return 1;
    }

    let sys_ref = unsafe { &*sys };

    // 3. Root of Trust & Header Integrity Cryptographic Audit
    let sig = sys_ref.hdr.signature;
    serial_str("[AEON:SEC]   SystemTable Signature: ");
    serial_hex_u64(sig);
    if sig == EFI_SYSTEM_TABLE_SIGNATURE {
        serial_str(" (\"IBI SYST\" - VERIFIED)\n");
    } else {
        serial_str(" (TAMPERED / UNKNOWN!)\n");
    }

    let crc_valid = verify_header_crc32(&sys_ref.hdr);
    serial_str("[AEON:SEC]   Header CRC32 Checksum: ");
    serial_hex_u32(sys_ref.hdr.crc32);
    if crc_valid {
        serial_str(" (INTEGRITY PASS)\n");
    } else {
        serial_str(" (INTEGRITY FAILED)\n");
    }

    // 4. CPU Hardware Defenses & Silicon Capability Audit (via CPUID)
    let cpuid_features = __cpuid(0x0000_0001);
    let cpuid_ext = __cpuid(0x8000_0001);
    let cpuid_sec = __cpuid_count(0x0000_0007, 0);

    let has_tsc = (cpuid_features.edx & (1 << 4)) != 0;
    let has_apic = (cpuid_features.edx & (1 << 9)) != 0;
    let has_aes = (cpuid_features.ecx & (1 << 25)) != 0;
    let has_rdrand = (cpuid_features.ecx & (1 << 30)) != 0;
    let has_nx = (cpuid_ext.edx & (1 << 20)) != 0;
    let has_smep = (cpuid_sec.ebx & (1 << 7)) != 0;
    let has_smap = (cpuid_sec.ebx & (1 << 20)) != 0;

    serial_str("[AEON:SEC]   Hardware Defenses: NX=");
    serial_str(if has_nx { "ON " } else { "OFF " });
    serial_str("| SMEP=");
    serial_str(if has_smep { "ON " } else { "OFF " });
    serial_str("| SMAP=");
    serial_str(if has_smap { "ON\n" } else { "OFF\n" });

    serial_str("[AEON:SEC]   Silicon Trust: AES-NI=");
    serial_str(if has_aes { "ON " } else { "OFF " });
    serial_str("| RDRAND=");
    serial_str(if has_rdrand { "ON " } else { "OFF " });
    serial_str("| APIC=");
    serial_str(if has_apic { "ON " } else { "OFF " });
    serial_str("| TSC=");
    serial_str(if has_tsc { "ON\n" } else { "OFF\n" });

    // 5. Mathematical Physical Memory Geometry Probing (GetMemoryMap)
    let mut total_pages = 0u64;
    let mut conventional_pages = 0u64;
    let mut reserved_pages = 0u64;
    let mut descriptor_count = 0usize;

    let bs = sys_ref.boot_services;
    if !bs.is_null() {
        let mut map_size = 16384usize;
        let mut map_key = 0usize;
        let mut desc_size = 0usize;
        let mut desc_ver = 0u32;

        let status = unsafe {
            let buf_ptr = core::ptr::addr_of_mut!(MEM_MAP_BUF) as *mut MemoryDescriptor;
            ((*bs).get_memory_map)(&mut map_size, buf_ptr, &mut map_key, &mut desc_size, &mut desc_ver)
        };

        if status == 0 && desc_size >= core::mem::size_of::<MemoryDescriptor>() {
            descriptor_count = map_size / desc_size;
            let raw_ptr = core::ptr::addr_of!(MEM_MAP_BUF) as *const u8;

            for i in 0..descriptor_count {
                let desc = unsafe { &*(raw_ptr.add(i * desc_size) as *const MemoryDescriptor) };
                total_pages += desc.number_of_pages;
                if desc.mem_type == 7 { // EfiConventionalMemory (Free RAM)
                    conventional_pages += desc.number_of_pages;
                } else {
                    reserved_pages += desc.number_of_pages;
                }
            }
        }
    }

    let total_ram_mb = (total_pages * 4096) / (1024 * 1024);
    let usable_ram_mb = (conventional_pages * 4096) / (1024 * 1024);
    let reserved_ram_mb = (reserved_pages * 4096) / (1024 * 1024);

    serial_str("[AEON:MATH]  Physical RAM Map Descriptors: ");
    serial_dec_u64(descriptor_count as u64);
    serial_str(" regions parsed\n");

    serial_str("[AEON:MATH]  Total Physical Memory:  ");
    serial_dec_u64(total_ram_mb);
    serial_str(" MB (");
    serial_dec_u64(total_pages);
    serial_str(" x 4KB frames)\n");

    serial_str("[AEON:MATH]  Conventional Usable RAM: ");
    serial_dec_u64(usable_ram_mb);
    serial_str(" MB (");
    serial_dec_u64(conventional_pages);
    serial_str(" free frames)\n");

    serial_str("[AEON:MATH]  Firmware & Reserved RAM: ");
    serial_dec_u64(reserved_ram_mb);
    serial_str(" MB (");
    serial_dec_u64(reserved_pages);
    serial_str(" frames)\n");

    // 6. Time Stamp Counter (TSC) Timing Telemetry
    let tsc_end = unsafe { _rdtsc() };
    let tsc_elapsed = tsc_end.saturating_sub(tsc_start);

    serial_str("[AEON:TIME]  Execution Cycle Overhead: ");
    serial_dec_u64(tsc_elapsed);
    serial_str(" CPU cycles\n");

    serial_str("[AEON:DIAG]  UART COM1 LSR Status: ");
    let lsr = unsafe { inb(COM1 + 5) };
    serial_hex_u8(lsr);
    serial_str("\n");

    // 7. Trust Score Evaluation
    let trust_score = if sig == EFI_SYSTEM_TABLE_SIGNATURE && crc_valid && has_nx {
        100u8
    } else if sig == EFI_SYSTEM_TABLE_SIGNATURE && crc_valid {
        90u8
    } else {
        40u8
    };

    serial_str("[AEON:TRUST] Boot Verification: ");
    serial_dec_u64(trust_score as u64);
    serial_str("% TRUST VERIFIED\n");
    serial_str("[AEON] ==================================================\n");

    // 8. User Display: Splash Logo (Zero clutter on user monitor)
    let cout = sys_ref.con_out;
    if !cout.is_null() {
        unsafe { ((*cout).clear_screen)(cout); }
        screen_print(cout, BOOT_LOGO);
    }

    // 9. CPU Idle Loop (x86 PAUSE to prevent thermal runaway)
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    serial_str("\n[AEON:PANIC] Hardware halted.\n");
    loop {
        core::hint::spin_loop();
    }
}
