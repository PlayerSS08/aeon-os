#![no_std]
#![no_main]

// ============================================================================
// 1. Minimal 64-Bit UEFI Firmware Interfaces
// ============================================================================

#[repr(C)]
pub struct TextOut {
    _reset: usize,
    pub output_string: extern "efiapi" fn(*mut TextOut, *const u16) -> usize,
    _test: usize,
    _query: usize,
    _set_mode: usize,
    pub set_attribute: extern "efiapi" fn(*mut TextOut, usize) -> usize,
    pub clear_screen: extern "efiapi" fn(*mut TextOut) -> usize,
}

#[repr(C)]
pub struct SystemTable {
    _hdr: [u8; 24],
    _vendor: *const u16,
    _rev: u32,
    _pad: u32,
    _in_h: *mut (),
    _in: *mut (),
    _out_h: *mut (),
    pub con_out: *mut TextOut,
}

// Centered AEON Unicode block art boot splash
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

fn screen_print(cout: *mut TextOut, s: &str) {
    if cout.is_null() {
        return;
    }
    let mut buf = [0u16; 128];
    let mut i = 0;
    for c in s.chars() {
        if i + 1 >= buf.len() {
            buf[i] = 0;
            unsafe {
                ((*cout).output_string)(cout, buf.as_ptr());
            }
            i = 0;
        }
        buf[i] = c as u16;
        i += 1;
    }
    if i > 0 {
        buf[i] = 0;
        unsafe {
            ((*cout).output_string)(cout, buf.as_ptr());
        }
    }
}

// ============================================================================
// 2. Minimal 16550 UART Serial Logging (COM1 @ 0x03F8)
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
        outb(COM1 + 4, 0x0B); // RTS/DSR
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

// ============================================================================
// 3. UEFI Boot Entry Point
// ============================================================================

#[no_mangle]
pub extern "efiapi" fn efi_main(_handle: *mut (), sys: *mut SystemTable) -> usize {
    if sys.is_null() {
        return 1;
    }
    let cout = unsafe { (*sys).con_out };
    if cout.is_null() {
        return 2;
    }

    // Initialize serial telemetry
    serial_init();
    serial_str("[AEON] Bootloader initialized (UEFI x86_64).\n");

    // Clear display & render centered AEON logo
    unsafe {
        ((*cout).clear_screen)(cout);
        ((*cout).set_attribute)(cout, 0x0F); // Bright white
    }
    screen_print(cout, BOOT_LOGO);

    serial_str("[AEON] Boot complete. Entering low-power halt.\n");

    // Low-power idle loop
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    serial_str("[AEON:PANIC] Kernel halted.\n");
    loop {
        core::hint::spin_loop();
    }
}
