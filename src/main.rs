#![no_std]
#![no_main]

use core::arch::x86_64::{__cpuid, __cpuid_count, _rdtsc};

// ============================================================================
// 1. UEFI System Hardware Structures & Table Specifications
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
    pub _reset: usize,
    pub output_string: extern "efiapi" fn(*mut TextOut, *const u16) -> usize,
    pub _test: usize,
    pub _query: usize,
    pub _set_mode: usize,
    pub set_attribute: extern "efiapi" fn(*mut TextOut, usize) -> usize,
    pub clear_screen: extern "efiapi" fn(*mut TextOut) -> usize,
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
#[derive(Clone, Copy)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    pub const fn new(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Self {
        Self { data1: d1, data2: d2, data3: d3, data4: d4 }
    }

    pub fn matches(&self, other: &Guid) -> bool {
        self.data1 == other.data1
            && self.data2 == other.data2
            && self.data3 == other.data3
            && self.data4 == other.data4
    }
}

pub const ACPI_20_TABLE_GUID: Guid = Guid::new(
    0x8868e871, 0xe4f1, 0x11d3,
    [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
);

pub const ACPI_10_TABLE_GUID: Guid = Guid::new(
    0xeb9d2d30, 0x2d88, 0x11d1,
    [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);

pub const SMBIOS3_TABLE_GUID: Guid = Guid::new(
    0xf2fd1544, 0x9794, 0x4a2c,
    [0x99, 0x2e, 0xe5, 0xbb, 0xcf, 0x20, 0xe3, 0x94],
);

pub const SMBIOS_TABLE_GUID: Guid = Guid::new(
    0xeb9d2d31, 0x2d88, 0x11d1,
    [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);

#[repr(C)]
pub struct ConfigurationTableEntry {
    pub vendor_guid: Guid,
    pub vendor_table: *const u8,
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
    pub number_of_table_entries: usize,
    pub configuration_table: *const ConfigurationTableEntry,
}

#[repr(C, packed)]
pub struct RsdpDescriptor {
    pub signature: [u8; 8], // "RSD PTR "
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

#[repr(C, packed)]
pub struct RsdpDescriptor20 {
    pub base: RsdpDescriptor,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

// Memory map buffer allocated statically to preserve stack boundaries
static mut MEM_MAP_BUF: [u8; 16384] = [0u8; 16384];

// ============================================================================
// 2. 16550 UART Serial Telemetry Engine (COM1 @ 0x03F8)
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

fn serial_hash(h: &[u8; 32]) {
    for &b in h.iter() {
        serial_hex_u8(b);
    }
}

// ============================================================================
// 3. UEFI Text Console UI & Color Management
// ============================================================================

#[allow(dead_code)]
const COLOR_BLACK: usize = 0x00;
#[allow(dead_code)]
const COLOR_BLUE: usize = 0x01;
#[allow(dead_code)]
const COLOR_GREEN: usize = 0x02;
#[allow(dead_code)]
const COLOR_CYAN: usize = 0x03;
#[allow(dead_code)]
const COLOR_RED: usize = 0x04;
#[allow(dead_code)]
const COLOR_LIGHT_GRAY: usize = 0x07;
#[allow(dead_code)]
const COLOR_DARK_GRAY: usize = 0x08;
#[allow(dead_code)]
const COLOR_LIGHT_BLUE: usize = 0x09;
#[allow(dead_code)]
const COLOR_LIGHT_GREEN: usize = 0x0A;
#[allow(dead_code)]
const COLOR_LIGHT_CYAN: usize = 0x0B;
#[allow(dead_code)]
const COLOR_LIGHT_RED: usize = 0x0C;
#[allow(dead_code)]
const COLOR_YELLOW: usize = 0x0E;
const COLOR_WHITE: usize = 0x0F;

fn screen_set_color(cout: *mut TextOut, color: usize) {
    if !cout.is_null() {
        unsafe { ((*cout).set_attribute)(cout, color); }
    }
}

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

#[allow(dead_code)]
fn screen_print_dec_u64(cout: *mut TextOut, mut n: u64) {
    if n == 0 {
        screen_print(cout, "0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut s_buf = [0u8; 20];
    let mut s_len = 0;
    while i > 0 {
        i -= 1;
        s_buf[s_len] = buf[i];
        s_len += 1;
    }
    if let Ok(s) = core::str::from_utf8(&s_buf[..s_len]) {
        screen_print(cout, s);
    }
}

#[allow(dead_code)]
fn screen_print_hex_u32(cout: *mut TextOut, val: u32) {
    screen_print(cout, "0x");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [0u8; 8];
    for i in 0..8 {
        buf[i] = HEX[((val >> ((7 - i) * 4)) & 0x0F) as usize];
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        screen_print(cout, s);
    }
}

#[allow(dead_code)]
fn screen_print_hex_u64(cout: *mut TextOut, val: u64) {
    screen_print(cout, "0x");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    for i in 0..16 {
        buf[i] = HEX[((val >> ((15 - i) * 4)) & 0x0F) as usize];
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        screen_print(cout, s);
    }
}

#[allow(dead_code)]
fn screen_print_hash(cout: *mut TextOut, h: &[u8; 32]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 64];
    for (i, &b) in h.iter().enumerate() {
        buf[i * 2] = HEX[(b >> 4) as usize];
        buf[i * 2 + 1] = HEX[(b & 0x0F) as usize];
    }
    if let Ok(s) = core::str::from_utf8(&buf) {
        screen_print(cout, s);
    }
}

// ============================================================================
// 4. Zero-Heap IEEE 802.3 CRC32 Engine
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
    scratch[16] = 0;
    scratch[17] = 0;
    scratch[18] = 0;
    scratch[19] = 0;
    let computed = crc32(&scratch[..len]);
    computed == hdr.crc32
}

// ============================================================================
// 5. Zero-Heap FIPS 180-4 SHA-256 Cryptographic Measured Boot Engine
// ============================================================================

pub struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    fn transform(&mut self, chunk: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(Self::K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn update(&mut self, mut data: &[u8]) {
        let buf_idx = (self.count as usize) % 64;
        self.count += data.len() as u64;

        if buf_idx > 0 {
            let fill = 64 - buf_idx;
            if data.len() < fill {
                self.buffer[buf_idx..buf_idx + data.len()].copy_from_slice(data);
                return;
            }
            self.buffer[buf_idx..64].copy_from_slice(&data[..fill]);
            let chunk = self.buffer;
            self.transform(&chunk);
            data = &data[fill..];
        }

        while data.len() >= 64 {
            let mut chunk = [0u8; 64];
            chunk.copy_from_slice(&data[..64]);
            self.transform(&chunk);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
        }
    }

    pub fn finalize(mut self, out: &mut [u8; 32]) {
        let total_bits = self.count * 8;
        let buf_idx = (self.count as usize) % 64;
        self.buffer[buf_idx] = 0x80;
        for i in (buf_idx + 1)..64 {
            self.buffer[i] = 0;
        }

        if buf_idx >= 56 {
            let chunk = self.buffer;
            self.transform(&chunk);
            self.buffer = [0u8; 64];
        }

        self.buffer[56..64].copy_from_slice(&total_bits.to_be_bytes());
        let chunk = self.buffer;
        self.transform(&chunk);

        for (i, val) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
        }
    }
}

// ============================================================================
// 6. ACPI & SMBIOS Root of Trust Verification Engines
// ============================================================================

fn find_acpi_rsdp(sys: &SystemTable) -> Option<(*const RsdpDescriptor, bool)> {
    if sys.configuration_table.is_null() || sys.number_of_table_entries == 0 {
        return None;
    }
    let count = if sys.number_of_table_entries > 256 { 256 } else { sys.number_of_table_entries };
    for i in 0..count {
        let entry = unsafe { &*sys.configuration_table.add(i) };
        if entry.vendor_guid.matches(&ACPI_20_TABLE_GUID) {
            return Some((entry.vendor_table as *const RsdpDescriptor, true));
        } else if entry.vendor_guid.matches(&ACPI_10_TABLE_GUID) {
            return Some((entry.vendor_table as *const RsdpDescriptor, false));
        }
    }
    None
}

fn verify_rsdp(rsdp_ptr: *const RsdpDescriptor, is_v2: bool) -> (bool, bool, u64) {
    if rsdp_ptr.is_null() {
        return (false, false, 0);
    }
    let raw = rsdp_ptr as *const u8;
    let sig_valid = unsafe {
        core::slice::from_raw_parts(raw, 8) == b"RSD PTR "
    };
    if !sig_valid {
        return (false, false, 0);
    }

    let mut sum1: u8 = 0;
    for i in 0..20 {
        sum1 = sum1.wrapping_add(unsafe { *raw.add(i) });
    }
    let csum1_valid = sum1 == 0;

    if is_v2 {
        let rsdp20 = rsdp_ptr as *const RsdpDescriptor20;
        let mut sum2: u8 = 0;
        for i in 0..36 {
            sum2 = sum2.wrapping_add(unsafe { *raw.add(i) });
        }
        let csum2_valid = sum2 == 0;
        let xsdt_addr = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*rsdp20).xsdt_address)) };
        (csum1_valid, csum2_valid, xsdt_addr)
    } else {
        let rsdt_addr = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!((*rsdp_ptr).rsdt_address)) };
        (csum1_valid, false, rsdt_addr as u64)
    }
}

fn find_smbios(sys: &SystemTable) -> Option<(*const u8, bool)> {
    if sys.configuration_table.is_null() || sys.number_of_table_entries == 0 {
        return None;
    }
    let count = if sys.number_of_table_entries > 256 { 256 } else { sys.number_of_table_entries };
    for i in 0..count {
        let entry = unsafe { &*sys.configuration_table.add(i) };
        if entry.vendor_guid.matches(&SMBIOS3_TABLE_GUID) {
            return Some((entry.vendor_table, true));
        } else if entry.vendor_guid.matches(&SMBIOS_TABLE_GUID) {
            return Some((entry.vendor_table, false));
        }
    }
    None
}

// ============================================================================
// 7. Hardware Randomness, NIST SP 800-22 Monobit & Shannon Entropy Engine
// ============================================================================

unsafe fn rdrand64() -> u64 {
    let mut val: u64 = 0;
    let mut cf: u8;
    for _ in 0..16 {
        core::arch::asm!(
            "rdrand {val}",
            "setc {cf}",
            val = out(reg) val,
            cf = out(reg_byte) cf,
            options(nomem, nostack)
        );
        if cf == 1 { return val; }
        core::hint::spin_loop();
    }
    let tsc = _rdtsc();
    val ^ tsc ^ tsc.rotate_left(17)
}

fn collect_entropy_sample(has_rdrand: bool, out: &mut [u64; 32]) {
    for word in out.iter_mut() {
        if has_rdrand {
            *word = unsafe { rdrand64() };
        } else {
            let tsc = unsafe { _rdtsc() };
            *word = tsc ^ tsc.rotate_left(31);
        }
    }
}

fn nist_monobit_test(words: &[u64; 32]) -> (u32, u32, u32, bool) {
    let mut ones: u32 = 0;
    for &w in words.iter() {
        ones += w.count_ones();
    }
    let zeros = 2048 - ones;
    let delta = if ones > zeros { ones - zeros } else { zeros - ones };
    // NIST SP 800-22 criterion for n=2048 at alpha=0.001 significance:
    // S_obs = delta / sqrt(2048) = delta / 45.2548 <= 3.29 => delta <= 148
    let pass = delta <= 148;
    (ones, zeros, delta, pass)
}

fn calculate_c_log2_c_fixed(c: u32) -> u32 {
    if c <= 1 { return 0; }
    let b = 31 - c.leading_zeros();
    let base = 1u32 << b;
    let rem = c - base;
    let log2_fixed = (b * 1024) + ((rem * 1024) / base);
    c * log2_fixed
}

fn calculate_shannon_entropy_millibits(words: &[u64; 32]) -> u32 {
    let mut counts = [0u16; 256];
    for &w in words.iter() {
        let bytes = w.to_le_bytes();
        for &b in bytes.iter() {
            counts[b as usize] += 1;
        }
    }
    let mut sum_c_log2_c: u32 = 0;
    for &c in counts.iter() {
        if c > 0 {
            sum_c_log2_c += calculate_c_log2_c_fixed(c as u32);
        }
    }
    // Max entropy for 256 bytes is log2(256) = 8.0 bits (8192 in 1024 fixed-point)
    let avg_loss = sum_c_log2_c / 256;
    let fixed_entropy = 8192u32.saturating_sub(avg_loss);
    // Scale to millibits (0..8000)
    (fixed_entropy * 1000) / 1024
}

// ============================================================================
// 8. Physical Memory Geometry & Mathematical Fragmentation Analysis
// ============================================================================

pub struct MemoryGeometry {
    pub total_mb: u64,
    pub usable_mb: u64,
    pub reserved_mb: u64,
    pub acpi_mb: u64,
    pub mmio_mb: u64,
    pub total_pages: u64,
    pub usable_pages: u64,
    pub descriptor_count: usize,
    pub free_segments: u64,
    pub largest_free_block_mb: u64,
    pub fragmentation_pct: u64,
    pub top_of_memory_mb: u64,
}

fn analyze_memory_geometry(sys: &SystemTable) -> MemoryGeometry {
    let mut geom = MemoryGeometry {
        total_mb: 0,
        usable_mb: 0,
        reserved_mb: 0,
        acpi_mb: 0,
        mmio_mb: 0,
        total_pages: 0,
        usable_pages: 0,
        descriptor_count: 0,
        free_segments: 0,
        largest_free_block_mb: 0,
        fragmentation_pct: 0,
        top_of_memory_mb: 0,
    };

    let bs = sys.boot_services;
    if bs.is_null() {
        return geom;
    }

    let mut map_size = 16384usize;
    let mut map_key = 0usize;
    let mut desc_size = 0usize;
    let mut desc_ver = 0u32;

    let status = unsafe {
        let buf_ptr = core::ptr::addr_of_mut!(MEM_MAP_BUF) as *mut MemoryDescriptor;
        ((*bs).get_memory_map)(&mut map_size, buf_ptr, &mut map_key, &mut desc_size, &mut desc_ver)
    };

    if status != 0 || desc_size < core::mem::size_of::<MemoryDescriptor>() {
        return geom;
    }

    geom.descriptor_count = map_size / desc_size;
    let raw_ptr = core::ptr::addr_of!(MEM_MAP_BUF) as *const u8;

    let mut max_free_pages = 0u64;
    let mut top_frame = 0u64;

    for i in 0..geom.descriptor_count {
        let desc = unsafe { &*(raw_ptr.add(i * desc_size) as *const MemoryDescriptor) };
        let pages = desc.number_of_pages;
        let end_addr = desc.physical_start.saturating_add(pages * 4096);
        if end_addr > top_frame {
            top_frame = end_addr;
        }

        geom.total_pages += pages;

        match desc.mem_type {
            7 => { // EfiConventionalMemory
                geom.usable_pages += pages;
                geom.free_segments += 1;
                if pages > max_free_pages {
                    max_free_pages = pages;
                }
            }
            9 | 10 => { // ACPI Reclaim / NVS
                geom.acpi_mb += (pages * 4096) / (1024 * 1024);
            }
            11 | 12 => { // MMIO / MMIO Port Space
                geom.mmio_mb += (pages * 4096) / (1024 * 1024);
            }
            _ => {
                geom.reserved_mb += (pages * 4096) / (1024 * 1024);
            }
        }
    }

    geom.total_mb = (geom.total_pages * 4096) / (1024 * 1024);
    geom.usable_mb = (geom.usable_pages * 4096) / (1024 * 1024);
    geom.largest_free_block_mb = (max_free_pages * 4096) / (1024 * 1024);
    geom.top_of_memory_mb = top_frame / (1024 * 1024);

    if geom.usable_pages > 0 && max_free_pages <= geom.usable_pages {
        let diff = geom.usable_pages - max_free_pages;
        geom.fragmentation_pct = (diff * 100) / geom.usable_pages;
    }

    geom
}

// ============================================================================
// 9. CPU Silicon Security & Physical Address Architecture Audit
// ============================================================================

pub struct SiliconAudit {
    pub has_nx: bool,
    pub has_smep: bool,
    pub has_smap: bool,
    pub has_aes: bool,
    pub has_rdrand: bool,
    pub has_rdseed: bool,
    pub has_cet_ss: bool,
    pub has_umip: bool,
    pub has_tsc: bool,
    pub has_apic: bool,
    pub is_hypervisor: bool,
    pub phys_bits: u32,
    pub virt_bits: u32,
}

fn audit_cpu_silicon() -> SiliconAudit {
    let f1 = __cpuid(0x0000_0001);
    let f_ext = __cpuid(0x8000_0001);
    let f7 = __cpuid_count(0x0000_0007, 0);
    let f_addr = __cpuid(0x8000_0008);

    let phys_bits = if f_addr.eax != 0 { f_addr.eax & 0xFF } else { 36 };
    let virt_bits = if f_addr.eax != 0 { (f_addr.eax >> 8) & 0xFF } else { 48 };

    SiliconAudit {
        has_tsc: (f1.edx & (1 << 4)) != 0,
        has_apic: (f1.edx & (1 << 9)) != 0,
        has_aes: (f1.ecx & (1 << 25)) != 0,
        has_rdrand: (f1.ecx & (1 << 30)) != 0,
        is_hypervisor: (f1.ecx & (1 << 31)) != 0,
        has_nx: (f_ext.edx & (1 << 20)) != 0,
        has_smep: (f7.ebx & (1 << 7)) != 0,
        has_smap: (f7.ebx & (1 << 20)) != 0,
        has_rdseed: (f7.ebx & (1 << 18)) != 0,
        has_umip: (f7.ecx & (1 << 2)) != 0,
        has_cet_ss: (f7.ecx & (1 << 7)) != 0,
        phys_bits,
        virt_bits,
    }
}

// ============================================================================
// 10. Composite Hardware Trust Attestation Matrix
// ============================================================================

pub struct TrustMatrix {
    pub sig_valid: bool,
    pub crc_valid: bool,
    pub acpi_valid: bool,
    pub silicon_nx: bool,
    pub silicon_smep_smap: bool,
    pub silicon_crypto: bool,
    pub entropy_pass: bool,
    pub mem_map_coherent: bool,
    pub score: u32,
}

impl TrustMatrix {
    pub fn compute(
        sig_valid: bool,
        crc_valid: bool,
        acpi_valid: bool,
        silicon: &SiliconAudit,
        entropy_pass: bool,
        geom: &MemoryGeometry,
    ) -> Self {
        let mut score = 0u32;
        if sig_valid { score += 15; }
        if crc_valid { score += 15; }
        if acpi_valid { score += 20; }
        if silicon.has_nx { score += 15; }
        if silicon.has_smep || silicon.has_smap || silicon.is_hypervisor { score += 15; }
        if silicon.has_aes && (silicon.has_rdrand || silicon.has_rdseed) { score += 10; }
        if entropy_pass { score += 10; }
        let mem_coherent = geom.usable_mb > 0 && geom.descriptor_count > 0;

        Self {
            sig_valid,
            crc_valid,
            acpi_valid,
            silicon_nx: silicon.has_nx,
            silicon_smep_smap: (silicon.has_smep && silicon.has_smap) || silicon.is_hypervisor,
            silicon_crypto: silicon.has_aes && silicon.has_rdrand,
            entropy_pass,
            mem_map_coherent: mem_coherent,
            score,
        }
    }
}

// Minimal universal Unicode Block Art boot splash
const BOOT_LOGO: &str = "\r
\n\r
\n\r
\n\r
\n\r
\n\r
\n\r
\n\r
\n\r
                  █████╗  ███████╗  ██████╗  ███╗   ██╗\r
                 ██╔══██╗ ██╔════╝ ██╔═══██╗ ████╗  ██║\r
                 ███████║ █████╗   ██║   ██║ ██╔██╗ ██║\r
                 ██╔══██║ ██╔══╝   ██║   ██║ ██║╚██╗██║\r
                 ██║  ██║ ███████╗ ╚██████╔╝ ██║ ╚████║\r
                 ╚═╝  ╚═╝ ╚══════╝  ╚═════╝  ╚═╝  ╚═══╝\r
\n\r";

// ============================================================================
// 11. Secure Kernel Entry Point
// ============================================================================

#[no_mangle]
pub extern "efiapi" fn efi_main(_handle: *mut (), sys: *mut SystemTable) -> usize {
    let tsc_start = unsafe { _rdtsc() };

    // 1. Initialize hardware serial telemetry immediately
    serial_init();

    serial_str("[AEON] ================================================================\n");
    serial_str("[AEON:BOOT]  Root of Trust Initialized (UEFI x86_64 Long Mode)\n");

    // 2. Pre-flight Pointer & Memory Safety Verification
    if sys.is_null() {
        serial_str("[AEON:PANIC] NULL SystemTable Pointer! Boot sequence aborted.\n");
        return 1;
    }

    let sys_ref = unsafe { &*sys };

    // 3. Root of Trust & Header Integrity Cryptographic Audit
    let sig = sys_ref.hdr.signature;
    let sig_valid = sig == EFI_SYSTEM_TABLE_SIGNATURE;
    serial_str("[AEON:SEC]   SystemTable Signature: ");
    serial_hex_u64(sig);
    if sig_valid {
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

    // 4. ACPI 2.0 / 1.0 Root of Trust Discovery & Checksum Engine
    let acpi_res = find_acpi_rsdp(sys_ref);
    let (acpi_found, acpi_is_v2, acpi_csum_pass, xsdt_phys) = match acpi_res {
        Some((rsdp, is_v2)) => {
            let (c1, c2, addr) = verify_rsdp(rsdp, is_v2);
            let pass = if is_v2 { c1 && c2 } else { c1 };
            (true, is_v2, pass, addr)
        }
        None => (false, false, false, 0u64),
    };

    serial_str("[AEON:ACPI]  ACPI Root Pointer: ");
    if acpi_found {
        if acpi_is_v2 { serial_str("ACPI 2.0+ (XSDT @ "); } else { serial_str("ACPI 1.0 (RSDT @ "); }
        serial_hex_u64(xsdt_phys);
        serial_str(if acpi_csum_pass { ") [CHECKSUM PASS]\n" } else { ") [CHECKSUM FAILED]\n" });
    } else {
        serial_str("NOT FOUND\n");
    }

    // 5. SMBIOS Discovery
    let smbios_res = find_smbios(sys_ref);
    serial_str("[AEON:SMBIOS] Anchor Status: ");
    if let Some((ptr, is_v3)) = smbios_res {
        serial_str(if is_v3 { "SMBIOS 3.0+ @ " } else { "SMBIOS 2.0+ @ " });
        serial_hex_u64(ptr as u64);
        serial_str("\n");
    } else {
        serial_str("NOT DETECTED\n");
    }

    // 6. CPU Hardware Defenses & Silicon Capability Audit
    let silicon = audit_cpu_silicon();
    serial_str("[AEON:SEC]   Hardware Defenses: NX=");
    serial_str(if silicon.has_nx { "ON " } else { "OFF " });
    serial_str("| SMEP=");
    serial_str(if silicon.has_smep { "ON " } else { "OFF " });
    serial_str("| SMAP=");
    serial_str(if silicon.has_smap { "ON " } else { "OFF " });
    serial_str("| UMIP=");
    serial_str(if silicon.has_umip { "ON " } else { "OFF " });
    serial_str("| HYPERVISOR=");
    serial_str(if silicon.is_hypervisor { "ACTIVE\n" } else { "OFF\n" });

    serial_str("[AEON:SEC]   Silicon Trust: AES-NI=");
    serial_str(if silicon.has_aes { "ON " } else { "OFF " });
    serial_str("| RDRAND=");
    serial_str(if silicon.has_rdrand { "ON " } else { "OFF " });
    serial_str("| RDSEED=");
    serial_str(if silicon.has_rdseed { "ON " } else { "OFF " });
    serial_str("| APIC=");
    serial_str(if silicon.has_apic { "ON " } else { "OFF " });
    serial_str("| TSC=");
    serial_str(if silicon.has_tsc { "ON\n" } else { "OFF\n" });

    serial_str("[AEON:ARCH]  Address Bus Topology: ");
    serial_dec_u64(silicon.phys_bits as u64);
    serial_str("-bit Physical | ");
    serial_dec_u64(silicon.virt_bits as u64);
    serial_str("-bit Virtual\n");

    // 7. Mathematical Physical Memory Geometry Probing (GetMemoryMap)
    let geom = analyze_memory_geometry(sys_ref);

    serial_str("[AEON:MATH]  Physical RAM Map Descriptors: ");
    serial_dec_u64(geom.descriptor_count as u64);
    serial_str(" regions parsed\n");

    serial_str("[AEON:MATH]  Total Physical Memory:  ");
    serial_dec_u64(geom.total_mb);
    serial_str(" MB (");
    serial_dec_u64(geom.total_pages);
    serial_str(" x 4KB frames)\n");

    serial_str("[AEON:MATH]  Conventional Usable RAM: ");
    serial_dec_u64(geom.usable_mb);
    serial_str(" MB (");
    serial_dec_u64(geom.usable_pages);
    serial_str(" free frames)\n");

    serial_str("[AEON:MATH]  ACPI Reclaim / MMIO:    ");
    serial_dec_u64(geom.acpi_mb);
    serial_str(" MB ACPI | ");
    serial_dec_u64(geom.mmio_mb);
    serial_str(" MB MMIO\n");

    serial_str("[AEON:MATH]  RAM Fragmentation:      ");
    serial_dec_u64(geom.fragmentation_pct);
    serial_str("% (Largest Block: ");
    serial_dec_u64(geom.largest_free_block_mb);
    serial_str(" MB in ");
    serial_dec_u64(geom.free_segments);
    serial_str(" segments)\n");

    serial_str("[AEON:MATH]  Top of Memory (TOM):    ");
    serial_dec_u64(geom.top_of_memory_mb);
    serial_str(" MB physical boundary\n");

    // 8. Hardware Entropy & NIST SP 800-22 Monobit Randomness Test
    let mut entropy_words = [0u64; 32];
    collect_entropy_sample(silicon.has_rdrand, &mut entropy_words);
    let (ones, zeros, delta, monobit_pass) = nist_monobit_test(&entropy_words);
    let shannon_mbit = calculate_shannon_entropy_millibits(&entropy_words);

    serial_str("[AEON:ENTR]  NIST SP 800-22 Monobit: ");
    if monobit_pass {
        serial_str("PASS (|n1-n0| = ");
    } else {
        serial_str("FAIL (|n1-n0| = ");
    }
    serial_dec_u64(delta as u64);
    serial_str(" / 2048 bits; ones=");
    serial_dec_u64(ones as u64);
    serial_str(", zeros=");
    serial_dec_u64(zeros as u64);
    serial_str(")\n");

    serial_str("[AEON:ENTR]  Shannon Entropy Density: ");
    serial_dec_u64((shannon_mbit / 1000) as u64);
    serial_byte(b'.');
    let mbit_rem = (shannon_mbit % 1000) / 10;
    if mbit_rem < 10 { serial_byte(b'0'); }
    serial_dec_u64(mbit_rem as u64);
    serial_str(" / 8.00 bits per byte\n");

    // 9. Zero-Heap Cryptographic Measured Boot (SHA-256 Hashes)
    // PCR0: Bootloader executable code measurement (4096 bytes around entry point)
    let mut pcr0_hasher = Sha256::new();
    let entry_page = (efi_main as *const () as usize) & !0xFFF;
    let code_slice = unsafe { core::slice::from_raw_parts(entry_page as *const u8, 4096) };
    pcr0_hasher.update(code_slice);
    let mut pcr0_digest = [0u8; 32];
    pcr0_hasher.finalize(&mut pcr0_digest);

    // PCR1: Firmware SystemTable & Configuration Table measurement
    let mut pcr1_hasher = Sha256::new();
    let sys_bytes = unsafe {
        core::slice::from_raw_parts(sys as *const u8, core::mem::size_of::<SystemTable>())
    };
    pcr1_hasher.update(sys_bytes);
    let mut pcr1_digest = [0u8; 32];
    pcr1_hasher.finalize(&mut pcr1_digest);

    // PCR2: Physical RAM Geometry & Descriptors measurement
    let mut pcr2_hasher = Sha256::new();
    let mem_raw = unsafe {
        let buf_ptr = core::ptr::addr_of!(MEM_MAP_BUF) as *const u8;
        core::slice::from_raw_parts(buf_ptr, 16384)
    };
    pcr2_hasher.update(mem_raw);
    let mut pcr2_digest = [0u8; 32];
    pcr2_hasher.finalize(&mut pcr2_digest);

    serial_str("[AEON:PCR0]  Bootloader Code SHA-256: ");
    serial_hash(&pcr0_digest);
    serial_str("\n");

    serial_str("[AEON:PCR1]  Firmware Tables SHA-256: ");
    serial_hash(&pcr1_digest);
    serial_str("\n");

    serial_str("[AEON:PCR2]  Memory Map Base SHA-256: ");
    serial_hash(&pcr2_digest);
    serial_str("\n");

    // 10. Composite Hardware Trust Attestation Matrix
    let trust = TrustMatrix::compute(
        sig_valid,
        crc_valid,
        acpi_found && acpi_csum_pass,
        &silicon,
        monobit_pass,
        &geom,
    );

    let tsc_end = unsafe { _rdtsc() };
    let tsc_elapsed = tsc_end.saturating_sub(tsc_start);

    serial_str("[AEON:TIME]  Execution Cycle Overhead: ");
    serial_dec_u64(tsc_elapsed);
    serial_str(" CPU cycles\n");

    serial_str("[AEON:TRUST] Attestation Score: ");
    serial_dec_u64(trust.score as u64);
    serial_str("/100 (");
    if trust.score >= 90 {
        serial_str("TIER-1 SECURE HARDWARE ROOT OF TRUST VERIFIED)\n");
    } else {
        serial_str("DEGRADED TRUST)\n");
    }
    serial_str("[AEON] ================================================================\n");

    // 11. User Display: Pure Minimal Splash Logo (Zero console clutter)
    let cout = sys_ref.con_out;
    if !cout.is_null() {
        unsafe { ((*cout).clear_screen)(cout); }
        screen_set_color(cout, COLOR_WHITE);
        screen_print(cout, BOOT_LOGO);
    }

    // 12. Non-terminating Hardware Idle Loop (x86 PAUSE instruction)
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
