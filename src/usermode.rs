//! Ring 3: the `syscall` entry, a small user window and the demo programs that run in it.
//!
//! Follows standard x86-64 Linux system call ABI. One user program runs at a time, in a window of
//! frames that are mapped user-accessible (code page read+execute, the rest read+write, never both).
//! Every other page of the kernel stays supervisor-only. Pointers from user code are checked against
//! the window (`usercopy`) and accessed with SMAP temporarily lifted (`stac`/`clac`).
//! A user fault or bad syscall ends only that task.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::arch::{self, ExcFrame};
use crate::global::Global;
use crate::mm;
use crate::paging;
use crate::ramfs;
use crate::sched;
use crate::syscall;
use crate::usercopy;
use crate::usbfs;

const WINDOW_PAGES: usize = 16;
const MAX_EXTRA_PAGES: usize = 32;
const TOTAL_ARENA_PAGES: usize = WINDOW_PAGES + MAX_EXTRA_PAGES;
const PAGE: u64 = 4096;
const NO_TASK: usize = usize::MAX;

const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;
const EFER_SCE: u64 = 1;
/// Cleared on `syscall` entry: TF, IF, DF, NT, AC (AC off keeps SMAP meaningful).
const SFMASK: u64 = 0x100 | 0x200 | 0x400 | 0x4000 | 0x4_0000;

/// Kernel stack the `syscall` entry switches to; kept equal to TSS.rsp0 for the running task.
#[unsafe(no_mangle)]
static AEON_SYSCALL_KSTACK: AtomicU64 = AtomicU64::new(0);
/// Holds the user stack pointer for the few instructions between `syscall` and the stack switch.
#[unsafe(no_mangle)]
static AEON_SYSCALL_SCRATCH: AtomicU64 = AtomicU64::new(0);

static WINDOW: AtomicU64 = AtomicU64::new(0);
static USER_BRK: AtomicU64 = AtomicU64::new(0);
static EXTRA_PAGES: AtomicUsize = AtomicUsize::new(0);
static EXTRA_PAGE_ADDRS: Global<[u64; MAX_EXTRA_PAGES]> = Global::new([0; MAX_EXTRA_PAGES]);

static ACTIVE: AtomicUsize = AtomicUsize::new(NO_TASK);
/// Set once `init` has enabled `syscall`; without it a ring-3 task could never make a call or exit.
static READY: AtomicBool = AtomicBool::new(false);

const MAX_FDS: usize = 16;

#[derive(Clone)]
struct OpenFile {
    path: String,
    offset: usize,
    flags: u32,
    is_usb: bool,
    is_socket: bool,
    _sock_domain: u32,
    sock_type: u32,
    _sock_protocol: u32,
    bound_port: u16,
    peer_ip: [u8; 4],
    peer_port: u16,
    connected: bool,
}

static FD_TABLE: Global<[Option<OpenFile>; MAX_FDS]> = Global::new([
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
]);

fn init_fds() {
    // SAFETY: single hardware thread, interrupts off during task startup.
    let table = unsafe { FD_TABLE.get() };
    table.fill(None);
}

/// Points both ring-3 entry paths (interrupts and `syscall`) at the incoming task's kernel stack.
///
/// # Safety
/// Interrupts must be off and `top` must be the 16-byte-aligned top of a mapped kernel stack.
pub unsafe fn set_kernel_stack(top: u64) {
    // SAFETY: forwarded contract.
    unsafe { arch::set_rsp0(top) };
    AEON_SYSCALL_KSTACK.store(top, Ordering::Relaxed);
}

global_asm!(
    r#"
    .text
    .p2align 4
    .globl aeon_syscall_entry
    aeon_syscall_entry:
        // rcx = user rip, r11 = user rflags, rax = number, rdi/rsi/rdx/r10/r8 = arguments, interrupts masked.
        mov [rip + AEON_SYSCALL_SCRATCH], rsp
        mov rsp, [rip + AEON_SYSCALL_KSTACK]
        push qword ptr [rip + AEON_SYSCALL_SCRATCH]   // user rsp lives on this task's own kernel stack
        push rcx
        push r11
        // Windows x64 ABI translation:
        //   rcx = n (from rax)
        //   rdx = a0 (from rdi)
        //   r8  = a1 (from rsi)
        //   r9  = a2 (from rdx)
        //   [rsp + 32] = a3 (from r10)
        //   [rsp + 40] = a4 (from r8)
        sub rsp, 48
        mov [rsp + 32], r10
        mov [rsp + 40], r8
        mov r9, rdx
        mov r8, rsi
        mov rdx, rdi
        mov rcx, rax
        cld
        call aeon_syscall_dispatch
        add rsp, 48
        // Do not leak kernel values in the registers the user ABI treats as clobbered.
        xor ecx, ecx
        xor edx, edx
        xor edi, edi
        xor esi, esi
        xor r8d, r8d
        xor r9d, r9d
        xor r10d, r10d
        pop r11
        pop rcx
        pop rsp
        sysretq

    // Demo programs, copied into the user window. Position independent.
    .p2align 4
    .globl aeon_user_hello_start
    aeon_user_hello_start:
        lea rdi, [rip + 2f]
        mov esi, 18
        mov eax, 1
        syscall
        xor edi, edi
        mov eax, 60
        syscall
    1:  jmp 1b
    2:  .ascii "hello from ring 3\n"
    3:
    .globl aeon_user_hello_end
    aeon_user_hello_end:

    .globl aeon_user_fault_start
    aeon_user_fault_start:
        mov eax, 0x100000
        mov qword ptr [rax], rax
    1:  jmp 1b
    .globl aeon_user_fault_end
    aeon_user_fault_end:

    .globl aeon_user_gp_start
    aeon_user_gp_start:
        hlt
    1:  jmp 1b
    .globl aeon_user_gp_end
    aeon_user_gp_end:

    .globl aeon_user_spin_start
    aeon_user_spin_start:
    1:  jmp 1b
    .globl aeon_user_spin_end
    aeon_user_spin_end:

    // Bad requests must fail cleanly: an unknown call number and a write of kernel memory both return
    // -1. The program exits with code 0 only if both did.
    .globl aeon_user_badcall_start
    aeon_user_badcall_start:
        mov eax, 99
        syscall
        mov rbx, rax
        mov edi, 0x100000
        mov esi, 8
        mov eax, 1
        syscall
        and rax, rbx
        add rax, 1
        mov rdi, rax
        mov eax, 60
        syscall
    1:  jmp 1b
    .globl aeon_user_badcall_end
    aeon_user_badcall_end:

    // Calls `uptime`, which this program is not allowed to make. The kernel must refuse (-1); the
    // program exits with code 0 only if it did, and with 1 if the call was obeyed.
    .globl aeon_user_denied_start
    aeon_user_denied_start:
        mov eax, 4
        syscall
        add rax, 1
        mov rdi, rax
        mov eax, 60
        syscall
    1:  jmp 1b
    .globl aeon_user_denied_end
    aeon_user_denied_end:

    // Linux-style userland program: dynamic memory (brk), file creation/write (open, write, close),
    // file reading (open, read, close), stdout print, and clean exit.
    .p2align 4
    .globl aeon_user_linux_start
    aeon_user_linux_start:
        // 1. brk(0) to get current heap break
        xor edi, edi
        mov eax, 12
        syscall
        mov rbx, rax

        // 2. brk(rbx + 4096) to expand heap
        lea rdi, [rbx + 4096]
        mov eax, 12
        syscall

        // 3. Write test string to heap
        lea rsi, [rip + 3f]
        mov rdi, rbx
        mov ecx, 26
        cld
        rep movsb

        // 4. open("/linux.txt", O_CREAT | O_RDWR = 0x42, 0644)
        lea rdi, [rip + 2f]
        mov esi, 0x42
        mov edx, 420
        mov eax, 2
        syscall
        mov r12, rax

        // 5. write(fd, rbx, 26)
        mov rdi, r12
        mov rsi, rbx
        mov edx, 26
        mov eax, 1
        syscall

        // 6. close(fd)
        mov rdi, r12
        mov eax, 3
        syscall

        // 7. open("/linux.txt", O_RDONLY = 0, 0)
        lea rdi, [rip + 2f]
        xor esi, esi
        xor edx, edx
        mov eax, 2
        syscall
        mov r12, rax

        // 8. read(fd, rbx + 1024, 26)
        mov rdi, r12
        lea rsi, [rbx + 1024]
        mov edx, 26
        xor eax, eax
        syscall
        mov r13, rax

        // 9. close(fd)
        mov rdi, r12
        mov eax, 3
        syscall

        // 10. write(1, rbx + 1024, r13) -> stdout
        mov edi, 1
        lea rsi, [rbx + 1024]
        mov rdx, r13
        mov eax, 1
        syscall

        // 11. exit(0)
        xor edi, edi
        mov eax, 60
        syscall
    1:  jmp 1b
    2:  .asciz "/linux.txt"
    3:  .ascii "hello from linux userland\n"
    .globl aeon_user_linux_end
    aeon_user_linux_end:
    "#
);

unsafe extern "C" {
    fn aeon_syscall_entry();
    static aeon_user_hello_start: u8;
    static aeon_user_hello_end: u8;
    static aeon_user_fault_start: u8;
    static aeon_user_fault_end: u8;
    static aeon_user_gp_start: u8;
    static aeon_user_gp_end: u8;
    static aeon_user_spin_start: u8;
    static aeon_user_spin_end: u8;
    static aeon_user_badcall_start: u8;
    static aeon_user_badcall_end: u8;
    static aeon_user_denied_start: u8;
    static aeon_user_denied_end: u8;
    static aeon_user_linux_start: u8;
    static aeon_user_linux_end: u8;
}

/// The built-in programs `run` can start.
pub const PROGRAMS: [&str; 7] = ["hello", "fault", "gp", "spin", "badcall", "denied", "linux"];

/// Which calls the running program may make (see `syscall::allow_mask`); set by `run`.
static ALLOW: AtomicU64 = AtomicU64::new(syscall::ALWAYS);

fn program(name: &str) -> Option<&'static [u8]> {
    // SAFETY: the symbols mark the start and end of byte ranges emitted by the global_asm! block above.
    unsafe {
        let (s, e) = match name {
            "hello" => (&aeon_user_hello_start as *const u8, &aeon_user_hello_end as *const u8),
            "fault" => (&aeon_user_fault_start as *const u8, &aeon_user_fault_end as *const u8),
            "gp" => (&aeon_user_gp_start as *const u8, &aeon_user_gp_end as *const u8),
            "spin" => (&aeon_user_spin_start as *const u8, &aeon_user_spin_end as *const u8),
            "badcall" => (&aeon_user_badcall_start as *const u8, &aeon_user_badcall_end as *const u8),
            "denied" => (&aeon_user_denied_start as *const u8, &aeon_user_denied_end as *const u8),
            "linux" => (&aeon_user_linux_start as *const u8, &aeon_user_linux_end as *const u8),
            _ => return None,
        };
        Some(core::slice::from_raw_parts(s, e as usize - s as usize))
    }
}

/// Turns on `syscall`/`sysret`. Call once after the GDT is loaded.
///
/// # Safety
/// `arch::init_descriptors` must have run (the selectors in STAR refer to its GDT), interrupts off.
pub unsafe fn init() {
    if !crate::boot::info().feat.smap {
        warn!("user", "no SMAP on this CPU: ring-3 programs are disabled");
        return;
    }
    // SAFETY: standard SYSCALL setup; the selectors match the GDT built by `init_descriptors`.
    unsafe {
        arch::wrmsr(arch::MSR_EFER, arch::rdmsr(arch::MSR_EFER) | EFER_SCE);
        arch::wrmsr(MSR_STAR, ((arch::SYSRET_BASE as u64) << 48) | ((arch::KERNEL_CS as u64) << 32));
        arch::wrmsr(MSR_LSTAR, aeon_syscall_entry as *const () as u64);
        arch::wrmsr(MSR_SFMASK, SFMASK);
    }
    READY.store(true, Ordering::Relaxed);
}

#[derive(Debug)]
pub enum RunError {
    Unknown,
    Busy,
    NoMemory,
    /// Recovery or single-task mode: no scheduler and no `syscall` setup.
    NotReady,
}

fn clean_extra_pages() {
    let count = EXTRA_PAGES.swap(0, Ordering::Relaxed);
    // SAFETY: single hardware thread, interrupts off during task cleanup.
    let addrs = unsafe { EXTRA_PAGE_ADDRS.get() };
    for slot in addrs.iter_mut().take(count.min(MAX_EXTRA_PAGES)) {
        let p = *slot;
        if p != 0 {
            let _ = paging::restore_page(p);
            *slot = 0;
        }
    }
}

/// Starts the named built-in program as a ring-3 task and returns its task id.
pub fn run(name: &str) -> Result<usize, RunError> {
    if !READY.load(Ordering::Relaxed) {
        return Err(RunError::NotReady);
    }
    let code = program(name).ok_or(RunError::Unknown)?;
    let active = ACTIVE.load(Ordering::Relaxed);
    if active != NO_TASK && sched::user_alive(active) {
        return Err(RunError::Busy);
    }

    clean_extra_pages();
    init_fds();

    let mut base = WINDOW.load(Ordering::Relaxed);
    if base == 0 {
        base = mm::alloc_frames(TOTAL_ARENA_PAGES).ok_or(RunError::NoMemory)?;
        WINDOW.store(base, Ordering::Relaxed);
    }
    USER_BRK.store(base + WINDOW_PAGES as u64 * PAGE, Ordering::Relaxed);

    // Wipe everything first, so nothing from an earlier program survives.
    for i in 0..WINDOW_PAGES as u64 {
        let page = base + i * PAGE;
        if !paging::restore_page(page) {
            return Err(RunError::NoMemory);
        }
        // SAFETY: the frames belong to the window, identity-mapped, writable and not user-visible here.
        unsafe { core::ptr::write_bytes(page as *mut u8, 0, PAGE as usize) };
    }
    // SAFETY: the code page is writable (just restored) and `code` is shorter than one page.
    unsafe { core::ptr::copy_nonoverlapping(code.as_ptr(), base as *mut u8, code.len().min(PAGE as usize)) };
    for i in 0..WINDOW_PAGES as u64 {
        let code_page = i == 0;
        if !paging::user_page(base + i * PAGE, !code_page, code_page) {
            return Err(RunError::NoMemory);
        }
    }
    let top = base + WINDOW_PAGES as u64 * PAGE - 16;
    ALLOW.store(syscall::allow_mask(name), Ordering::Relaxed);
    let id = sched::spawn_user(name, base, top).ok_or(RunError::NoMemory)?;
    ACTIVE.store(id, Ordering::Relaxed);
    Ok(id)
}

fn user_window() -> (u64, u64) {
    let b = WINDOW.load(Ordering::Relaxed);
    let brk = USER_BRK.load(Ordering::Relaxed);
    (b, brk)
}

/// Copies `len` bytes from user memory into `buf`, or returns false if the range is not user memory.
fn copy_from_user(ptr: u64, buf: &mut [u8]) -> bool {
    let (s, e) = user_window();
    if s == 0 || !usercopy::check(s, e, ptr, buf.len() as u64) {
        return false;
    }
    // SAFETY: the range is inside the user window, which is mapped. SMAP is lifted only for the copy.
    unsafe {
        asm!("stac", options(nomem, nostack));
        core::ptr::copy_nonoverlapping(ptr as *const u8, buf.as_mut_ptr(), buf.len());
        asm!("clac", options(nomem, nostack));
    }
    true
}

/// Copies `src` bytes from kernel into user memory at `ptr`. Cannot write to user code page 0.
fn copy_to_user(ptr: u64, src: &[u8]) -> bool {
    let (s, e) = user_window();
    if s == 0 || !usercopy::check_write(s, e, ptr, src.len() as u64, PAGE) {
        return false;
    }
    // SAFETY: the range is inside the user writable window, mapped. SMAP is lifted only for the copy.
    unsafe {
        asm!("stac", options(nomem, nostack));
        core::ptr::copy_nonoverlapping(src.as_ptr(), ptr as *mut u8, src.len());
        asm!("clac", options(nomem, nostack));
    }
    true
}

fn copy_str_from_user(ptr: u64, max_len: usize) -> Option<String> {
    let (s, e) = user_window();
    if s == 0 || ptr < s || ptr >= e {
        return None;
    }
    let mut bytes = Vec::new();
    let mut p = ptr;
    while p < e && bytes.len() < max_len {
        let mut b = [0u8; 1];
        if !copy_from_user(p, &mut b) {
            return None;
        }
        if b[0] == 0 {
            break;
        }
        bytes.push(b[0]);
        p += 1;
    }
    String::from_utf8(bytes).ok()
}

fn sys_open(path_ptr: u64, flags: u32, _mode: u32) -> u64 {
    let Some(raw_path) = copy_str_from_user(path_ptr, ramfs::MAX_PATH) else {
        return syscall::errno(syscall::EFAULT);
    };
    let Some(path) = ramfs::resolve("/", &raw_path) else {
        return syscall::errno(syscall::EINVAL);
    };
    let is_usb = usbfs::is_usb(&path);
    if is_usb {
        let rel = usbfs::rel_path(&path);
        if flags & (syscall::O_WRONLY | syscall::O_RDWR | syscall::O_CREAT) != 0
            && usbfs::check_write_safety(rel).is_err()
        {
            return syscall::errno(syscall::EACCES);
        }
    } else {
        let fs = crate::shell::fs();
        let exists = fs.read(&path).is_some();
        if !exists {
            if flags & syscall::O_CREAT != 0 {
                if fs.write(&path, b"").is_err() {
                    return syscall::errno(syscall::ENOSPC);
                }
            } else {
                return syscall::errno(syscall::ENOENT);
            }
        }
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    for (fd, slot) in table.iter_mut().enumerate().take(MAX_FDS).skip(3) {
        if slot.is_none() {
            *slot = Some(OpenFile {
                path,
                offset: 0,
                flags,
                is_usb,
                is_socket: false,
                _sock_domain: 0,
                sock_type: 0,
                _sock_protocol: 0,
                bound_port: 0,
                peer_ip: [0; 4],
                peer_port: 0,
                connected: false,
            });
            return fd as u64;
        }
    }
    syscall::errno(syscall::ENOMEM)
}

fn sys_close(fd: u64) -> u64 {
    let fd = fd as usize;
    if fd < 3 {
        return 0;
    }
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    if table[fd].take().is_some() {
        0
    } else {
        syscall::errno(syscall::EBADF)
    }
}

fn sys_read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let fd = fd as usize;
    if fd == 0 {
        // Stdin: non-blocking or EOF for scripted userland
        return 0;
    }
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    let count = count.min(65536) as usize;
    if count == 0 {
        return 0;
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) => f,
        None => return syscall::errno(syscall::EBADF),
    };
    if file.is_socket {
        return sys_recvfrom(fd as u64, buf_ptr, count as u64, 0, 0, 0);
    }
    if file.flags & syscall::O_WRONLY != 0 && file.flags & syscall::O_RDWR == 0 {
        return syscall::errno(syscall::EBADF);
    }
    if file.is_usb {
        let rel = usbfs::rel_path(&file.path);
        let data = match usbfs::hw_read(rel, 1024 * 1024) {
            Ok(d) => d,
            Err(_) => return syscall::errno(syscall::ENOENT),
        };
        if file.offset >= data.len() {
            return 0;
        }
        let avail = &data[file.offset..];
        let to_read = avail.len().min(count);
        if !copy_to_user(buf_ptr, &avail[..to_read]) {
            return syscall::errno(syscall::EFAULT);
        }
        file.offset += to_read;
        to_read as u64
    } else {
        let fs = crate::shell::fs();
        let Some(data) = fs.read(&file.path) else {
            return syscall::errno(syscall::ENOENT);
        };
        if file.offset >= data.len() {
            return 0;
        }
        let avail = &data[file.offset..];
        let to_read = avail.len().min(count);
        if !copy_to_user(buf_ptr, &avail[..to_read]) {
            return syscall::errno(syscall::EFAULT);
        }
        file.offset += to_read;
        to_read as u64
    }
}

fn sys_write(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    // Legacy support: if fd >= 1024, caller passed write(buf, count)
    if fd >= 1024 {
        let len = buf_ptr.min(syscall::MAX_WRITE);
        let mut buf = [0u8; 256];
        let chunk = len.min(buf.len() as u64) as usize;
        if !copy_from_user(fd, &mut buf[..chunk]) {
            return syscall::ERR;
        }
        syscall::sanitize_output(&mut buf[..chunk]);
        crate::log::print_raw(&buf[..chunk]);
        return chunk as u64;
    }
    let fd = fd as usize;
    if fd == 1 || fd == 2 {
        let to_write = count.min(syscall::MAX_WRITE) as usize;
        let mut buf = alloc::vec![0u8; to_write];
        if !copy_from_user(buf_ptr, &mut buf) {
            return syscall::errno(syscall::EFAULT);
        }
        syscall::sanitize_output(&mut buf);
        crate::log::print_raw(&buf);
        return to_write as u64;
    }
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) => f,
        None => return syscall::errno(syscall::EBADF),
    };
    if file.is_socket {
        return sys_sendto(fd as u64, buf_ptr, count, 0, 0, 0);
    }
    if file.flags & (syscall::O_WRONLY | syscall::O_RDWR) == 0 && file.flags != 0 {
        return syscall::errno(syscall::EBADF);
    }
    let to_write = count.min(1024 * 1024) as usize;
    let mut buf = alloc::vec![0u8; to_write];
    if !copy_from_user(buf_ptr, &mut buf) {
        return syscall::errno(syscall::EFAULT);
    }
    if file.is_usb {
        let rel = usbfs::rel_path(&file.path);
        let mut cur = usbfs::hw_read(rel, 1024 * 1024).unwrap_or_default();
        if file.offset + to_write > cur.len() {
            cur.resize(file.offset + to_write, 0);
        }
        cur[file.offset..file.offset + to_write].copy_from_slice(&buf);
        if usbfs::hw_write(rel, &cur, true).is_err() {
            return syscall::errno(syscall::ENOSPC);
        }
        file.offset += to_write;
        to_write as u64
    } else {
        let fs = crate::shell::fs();
        let mut cur = fs.read(&file.path).unwrap_or(b"").to_vec();
        if file.offset + to_write > cur.len() {
            cur.resize(file.offset + to_write, 0);
        }
        cur[file.offset..file.offset + to_write].copy_from_slice(&buf);
        if fs.write(&file.path, &cur).is_err() {
            return syscall::errno(syscall::ENOSPC);
        }
        file.offset += to_write;
        to_write as u64
    }
}

fn sys_lseek(fd: u64, offset: i64, whence: u32) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) => f,
        None => return syscall::errno(syscall::EBADF),
    };
    let file_len = if file.is_usb {
        let rel = usbfs::rel_path(&file.path);
        usbfs::hw_read(rel, 1024 * 1024).map(|d| d.len()).unwrap_or(0)
    } else {
        crate::shell::fs().read(&file.path).map(|d| d.len()).unwrap_or(0)
    };
    let new_off: i64 = match whence {
        syscall::SEEK_SET => offset,
        syscall::SEEK_CUR => file.offset as i64 + offset,
        syscall::SEEK_END => file_len as i64 + offset,
        _ => return syscall::errno(syscall::EINVAL),
    };
    if new_off < 0 {
        return syscall::errno(syscall::EINVAL);
    }
    file.offset = new_off as usize;
    file.offset as u64
}

fn sys_brk(addr: u64) -> u64 {
    let base = WINDOW.load(Ordering::Relaxed);
    let initial_top = base + WINDOW_PAGES as u64 * PAGE;
    let curr = USER_BRK.load(Ordering::Relaxed);
    if addr == 0 {
        return curr;
    }
    if addr <= curr {
        if addr >= initial_top {
            USER_BRK.store(addr, Ordering::Relaxed);
            return addr;
        }
        return curr;
    }
    let max_top = initial_top + (MAX_EXTRA_PAGES as u64) * PAGE;
    if addr > max_top {
        return curr;
    }
    let target_page = (addr + PAGE - 1) & !(PAGE - 1);
    let curr_page = (curr + PAGE - 1) & !(PAGE - 1);
    let mut p = curr_page;
    while p < target_page {
        if !paging::user_page(p, true, false) {
            return curr;
        }
        // SAFETY: `p` is a newly mapped user page frame, identity mapped in kernel space.
        unsafe { core::ptr::write_bytes(p as *mut u8, 0, PAGE as usize) };
        let count = EXTRA_PAGES.load(Ordering::Relaxed);
        if count < MAX_EXTRA_PAGES {
            // SAFETY: single hardware thread, interrupts off during syscall dispatch.
            let addrs = unsafe { EXTRA_PAGE_ADDRS.get() };
            addrs[count] = p;
            EXTRA_PAGES.store(count + 1, Ordering::Relaxed);
        }
        p += PAGE;
    }
    USER_BRK.store(addr, Ordering::Relaxed);
    addr
}

fn sys_mmap(_addr: u64, length: u64, _prot: u64, flags: u64, _fd: u64, _offset: u64) -> u64 {
    if length == 0 {
        return syscall::errno(syscall::EINVAL);
    }
    if flags & syscall::MAP_ANONYMOUS as u64 != 0 {
        let curr = USER_BRK.load(Ordering::Relaxed);
        let new_brk = (curr + length + PAGE - 1) & !(PAGE - 1);
        let res = sys_brk(new_brk);
        if res == new_brk {
            curr
        } else {
            syscall::errno(syscall::ENOMEM)
        }
    } else {
        syscall::errno(syscall::EINVAL)
    }
}

fn sys_socket(domain: u32, sock_type: u32, protocol: u32) -> u64 {
    if domain != syscall::AF_INET && domain != 0 {
        return syscall::errno(syscall::EAFNOSUPPORT);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    for (fd, slot) in table.iter_mut().enumerate().take(MAX_FDS).skip(3) {
        if slot.is_none() {
            *slot = Some(OpenFile {
                path: String::from("[socket]"),
                offset: 0,
                flags: syscall::O_RDWR,
                is_usb: false,
                is_socket: true,
                _sock_domain: domain,
                sock_type,
                _sock_protocol: protocol,
                bound_port: 0,
                peer_ip: [0; 4],
                peer_port: 0,
                connected: false,
            });
            return fd as u64;
        }
    }
    syscall::errno(syscall::ENOMEM)
}

fn sys_connect(fd: u64, addr_ptr: u64, addr_len: u64) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    if addr_len < 8 {
        return syscall::errno(syscall::EINVAL);
    }
    let mut sock_buf = [0u8; 16];
    if !copy_from_user(addr_ptr, &mut sock_buf[..addr_len.min(16) as usize]) {
        return syscall::errno(syscall::EFAULT);
    }
    let port = u16::from_be_bytes([sock_buf[2], sock_buf[3]]);
    let ip = [sock_buf[4], sock_buf[5], sock_buf[6], sock_buf[7]];

    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) if f.is_socket => f,
        _ => return syscall::errno(syscall::EBADF),
    };

    file.peer_ip = ip;
    file.peer_port = port;
    file.connected = true;
    0
}

fn sys_bind(fd: u64, addr_ptr: u64, addr_len: u64) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    if addr_len < 4 {
        return syscall::errno(syscall::EINVAL);
    }
    let mut sock_buf = [0u8; 16];
    if !copy_from_user(addr_ptr, &mut sock_buf[..addr_len.min(16) as usize]) {
        return syscall::errno(syscall::EFAULT);
    }
    let port = u16::from_be_bytes([sock_buf[2], sock_buf[3]]);

    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) if f.is_socket => f,
        _ => return syscall::errno(syscall::EBADF),
    };

    file.bound_port = port;
    0
}

fn sys_listen(fd: u64, _backlog: u32) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    match &mut table[fd] {
        Some(f) if f.is_socket => 0,
        _ => syscall::errno(syscall::EBADF),
    }
}

fn sys_sendto(fd: u64, buf_ptr: u64, count: u64, _flags: u32, dst_ptr: u64, _dst_len: u32) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    let to_send = count.min(1500) as usize;
    let mut buf = alloc::vec![0u8; to_send];
    if !copy_from_user(buf_ptr, &mut buf) {
        return syscall::errno(syscall::EFAULT);
    }

    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let file = match &mut table[fd] {
        Some(f) if f.is_socket => f,
        _ => return syscall::errno(syscall::EBADF),
    };

    let target_ip = if dst_ptr != 0 {
        let mut sock_buf = [0u8; 16];
        if !copy_from_user(dst_ptr, &mut sock_buf[..8]) {
            return syscall::errno(syscall::EFAULT);
        }
        crate::netstack::Ipv4Addr([sock_buf[4], sock_buf[5], sock_buf[6], sock_buf[7]])
    } else {
        crate::netstack::Ipv4Addr(file.peer_ip)
    };

    let proto = if file.sock_type == syscall::SOCK_DGRAM {
        crate::netstack::IPPROTO_UDP
    } else {
        crate::netstack::IPPROTO_TCP
    };

    match crate::netstack::send_ipv4_packet(target_ip, proto, &buf) {
        Ok(()) => to_send as u64,
        Err(_) => syscall::errno(syscall::ENETUNREACH),
    }
}

fn sys_recvfrom(fd: u64, buf_ptr: u64, count: u64, _flags: u32, src_ptr: u64, src_len_ptr: u64) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return syscall::errno(syscall::EBADF);
    }
    // SAFETY: single hardware thread, interrupts off during syscall dispatch.
    let table = unsafe { FD_TABLE.get() };
    let _file = match &mut table[fd] {
        Some(f) if f.is_socket => f,
        _ => return syscall::errno(syscall::EBADF),
    };

    let mut buf = [0u8; 1500];
    crate::netstack::poll();
    match crate::netdev::receive_packet(&mut buf) {
        Ok(len) if len > 0 => {
            let to_copy = (count as usize).min(len);
            if !copy_to_user(buf_ptr, &buf[..to_copy]) {
                return syscall::errno(syscall::EFAULT);
            }
            if src_ptr != 0 && src_len_ptr != 0 {
                let sock_in = [2u8, 0, 0, 80, 10, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0];
                let _ = copy_to_user(src_ptr, &sock_in);
            }
            to_copy as u64
        }
        _ => 0,
    }
}

/// Rust side of the `syscall` entry. Runs with interrupts off on the task's kernel stack.
#[unsafe(no_mangle)]
extern "C" fn aeon_syscall_dispatch(n: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> u64 {
    let mask = ALLOW.load(Ordering::Relaxed);
    if !syscall::permits_raw(mask, n, a0) {
        return syscall::ERR;
    }
    match n {
        0 => sys_read(a0, a1, a2),
        1 => sys_write(a0, a1, a2),
        2 => {
            if mask & syscall::Number::Open.bit() != 0 {
                sys_open(a0, a1 as u32, a2 as u32)
            } else if a0 == 0 && mask & syscall::LEGACY_EXIT_BIT != 0 {
                crate::log::print_raw(b"[user] task ");
                print_num(sched::current_id() as u64);
                crate::log::print_raw(b" exited with code 0\n");
                sched::exit()
            } else {
                sys_open(a0, a1 as u32, a2 as u32)
            }
        }
        3 => {
            if mask & syscall::Number::Close.bit() != 0 {
                sys_close(a0)
            } else if a0 < 3 && mask & syscall::LEGACY_YIELD_BIT != 0 {
                sched::yield_now();
                0
            } else {
                sys_close(a0)
            }
        }
        4 => crate::log::uptime_us(),
        5 => sched::current_id() as u64,
        8 => sys_lseek(a0, a1 as i64, a2 as u32),
        9 => sys_mmap(a0, a1, a2, a3, a4, 0),
        12 => sys_brk(a0),
        24 => {
            sched::yield_now();
            0
        }
        39 => sched::current_id() as u64,
        41 => sys_socket(a0 as u32, a1 as u32, a2 as u32),
        42 => sys_connect(a0, a1, a2),
        44 => sys_sendto(a0, a1, a2, a3 as u32, a4, 0),
        45 => sys_recvfrom(a0, a1, a2, a3 as u32, a4, 0),
        49 => sys_bind(a0, a1, a2),
        50 => sys_listen(a0, a1 as u32),
        60 => {
            crate::log::print_raw(b"[user] task ");
            print_num(sched::current_id() as u64);
            crate::log::print_raw(b" exited with code ");
            print_num(a0);
            crate::log::print_raw(b"\n");
            sched::exit()
        }
        228 => {
            let us = crate::log::uptime_us();
            if a1 != 0 {
                let sec = us / 1_000_000;
                let nsec = (us % 1_000_000) * 1000;
                let mut ts = [0u8; 16];
                ts[..8].copy_from_slice(&sec.to_le_bytes());
                ts[8..].copy_from_slice(&nsec.to_le_bytes());
                if !copy_to_user(a1, &ts) {
                    return syscall::errno(syscall::EFAULT);
                }
            }
            0
        }
        _ => syscall::ERR,
    }
}

fn print_num(mut v: u64) {
    let mut b = [0u8; 20];
    let mut i = b.len();
    loop {
        i -= 1;
        b[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    crate::log::print_raw(&b[i..]);
}

/// Called from the exception entry when the CPU faulted in ring 3: report it and end the task.
pub fn user_fault(f: &ExcFrame, name: &str, cr2: u64) -> ! {
    warn!(
        "user",
        "task {} killed: {} at rip={:#x} error_code={:#x} cr2={:#x}",
        sched::current_id(),
        name,
        f.rip,
        f.error_code,
        cr2
    );
    crate::log::print_raw(b"[user] task killed: ");
    crate::log::print_raw(name.as_bytes());
    crate::log::print_raw(b"\n");
    sched::exit()
}
