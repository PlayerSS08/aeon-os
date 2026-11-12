//! System call numbers. Pure, host-testable. Calling convention: number in `rax`, arguments in `rdi`,
//! `rsi`, `rdx`, `r10`, `r8`, `r9`, result in `rax`, issued with the `syscall` instruction.
//!
//! Follows the standard x86-64 Linux system call ABI.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Number {
    /// `read(fd, buf, count)`: Linux sys_read
    Read = 0,
    /// `write(fd, buf, count)`: Linux sys_write (also legacy `write(buf, count)`)
    Write = 1,
    /// `open(path, flags, mode)`: Linux sys_open
    Open = 2,
    /// `close(fd)`: Linux sys_close
    Close = 3,
    /// `lseek(fd, offset, whence)`: Linux sys_lseek
    Lseek = 8,
    /// `mmap(addr, len, prot, flags, fd, offset)`: Linux sys_mmap
    Mmap = 9,
    /// `brk(addr)`: Linux sys_brk
    Brk = 12,
    /// `sched_yield()`: Linux sys_sched_yield
    Yield = 24,
    /// `getpid()`: Linux sys_getpid
    GetPid = 39,
    /// `exit(code)`: Linux sys_exit
    Exit = 60,
    /// `clock_gettime(clk_id, tp)`: Linux sys_clock_gettime (uptime)
    ClockGetTime = 228,
}

pub const MAX_WRITE: u64 = 4096;
/// Returned for bad pointers, bad lengths and unknown calls.
pub const ERR: u64 = u64::MAX;

// Standard Linux file open flags
pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;
pub const O_RDWR: u32 = 2;
pub const O_CREAT: u32 = 0x40;
pub const O_APPEND: u32 = 0x400;

// Standard Linux lseek whence
pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

// Standard Linux mmap protection and flags
pub const PROT_READ: u32 = 1;
pub const PROT_WRITE: u32 = 2;
pub const PROT_EXEC: u32 = 4;
pub const MAP_SHARED: u32 = 1;
pub const MAP_PRIVATE: u32 = 2;
pub const MAP_ANONYMOUS: u32 = 0x20;

// Standard Linux errno values
pub const EPERM: u64 = 1;
pub const ENOENT: u64 = 2;
pub const EBADF: u64 = 9;
pub const ENOMEM: u64 = 12;
pub const EACCES: u64 = 13;
pub const EFAULT: u64 = 14;
pub const EINVAL: u64 = 22;
pub const ENOSPC: u64 = 28;

pub const fn errno(code: u64) -> u64 {
    (0u64).wrapping_sub(code)
}

/// Makes bytes from a user program safe to print: everything except printable ASCII, newline and tab
/// becomes `?`, so a program cannot forge log lines with carriage returns or send terminal escapes.
pub fn sanitize_output(buf: &mut [u8]) {
    for b in buf {
        if !(matches!(*b, b'\n' | b'\t') || (0x20..0x7F).contains(b)) {
            *b = b'?';
        }
    }
}

pub const LEGACY_EXIT_BIT: u64 = 1 << 62;
pub const LEGACY_YIELD_BIT: u64 = 1 << 61;

impl Number {
    /// This call's bit in an allow-mask.
    pub const fn bit(self) -> u64 {
        match self {
            Number::ClockGetTime => 1 << 63,
            _ => 1 << (self as u64),
        }
    }
}

/// Calls every program may make: without `exit` a program could never end cleanly.
/// Includes Linux sys_exit (60) and legacy exit (2 with code 0).
pub const ALWAYS: u64 = Number::Exit.bit() | LEGACY_EXIT_BIT;

/// The calls a built-in program is allowed to make (least privilege: `hello` may print, `denied`
/// may only exit). Unknown program names get `exit` only.
pub fn allow_mask(program: &str) -> u64 {
    ALWAYS
        | match program {
            "hello" | "badcall" => Number::Write.bit(),
            "spin" => Number::Yield.bit() | LEGACY_YIELD_BIT,
            "linux" => {
                Number::Read.bit()
                    | Number::Write.bit()
                    | Number::Open.bit()
                    | Number::Close.bit()
                    | Number::Lseek.bit()
                    | Number::Mmap.bit()
                    | Number::Brk.bit()
                    | Number::Yield.bit()
                    | Number::GetPid.bit()
                    | Number::ClockGetTime.bit()
            }
            _ => 0,
        }
}

/// True if `mask` lets the program make call `n`.
pub const fn permits(mask: u64, n: Number) -> bool {
    mask & n.bit() != 0
}

/// True if raw call number `n` is permitted by `mask` (including legacy alias numbers).
pub fn permits_raw(mask: u64, n: u64, a0: u64) -> bool {
    match n {
        2 if a0 == 0 => mask & LEGACY_EXIT_BIT != 0, // legacy exit
        2 => mask & Number::Open.bit() != 0,         // Linux open
        3 if a0 < 3 => mask & LEGACY_YIELD_BIT != 0, // legacy yield
        3 => mask & Number::Close.bit() != 0,        // Linux close
        4 => mask & Number::ClockGetTime.bit() != 0, // legacy uptime
        5 => mask & Number::GetPid.bit() != 0,       // legacy getpid
        _ => {
            if let Some(num) = decode(n) {
                permits(mask, num)
            } else {
                false
            }
        }
    }
}

pub fn decode(n: u64) -> Option<Number> {
    Some(match n {
        0 => Number::Read,
        1 => Number::Write,
        2 => Number::Open,
        3 => Number::Close,
        8 => Number::Lseek,
        9 => Number::Mmap,
        12 => Number::Brk,
        24 => Number::Yield,
        39 => Number::GetPid,
        60 => Number::Exit,
        228 => Number::ClockGetTime,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_roundtrip() {
        let all = [
            Number::Read,
            Number::Write,
            Number::Open,
            Number::Close,
            Number::Lseek,
            Number::Mmap,
            Number::Brk,
            Number::Yield,
            Number::GetPid,
            Number::Exit,
            Number::ClockGetTime,
        ];
        for n in all {
            assert_eq!(decode(n as u64), Some(n));
        }
    }

    #[test]
    fn allow_masks_are_least_privilege() {
        let all = [
            Number::Read,
            Number::Write,
            Number::Open,
            Number::Close,
            Number::Lseek,
            Number::Mmap,
            Number::Brk,
            Number::Yield,
            Number::GetPid,
            Number::Exit,
            Number::ClockGetTime,
        ];
        // Every program can exit, and nothing can make a call outside its list.
        for p in ["hello", "fault", "gp", "spin", "badcall", "denied", "whatever"] {
            assert!(permits(allow_mask(p), Number::Exit), "{p} must be able to exit");
            assert!(!permits(allow_mask(p), Number::ClockGetTime), "{p} must not read the clock");
            assert!(!permits(allow_mask(p), Number::GetPid), "{p} must not read its id");
        }
        assert!(permits(allow_mask("hello"), Number::Write));
        assert!(!permits(allow_mask("hello"), Number::Yield));
        assert!(permits(allow_mask("spin"), Number::Yield));
        assert!(!permits(allow_mask("spin"), Number::Write));
        // `linux` program can perform all Linux calls
        for n in all {
            assert!(permits(allow_mask("linux"), n), "linux must permit {n:?}");
        }
        // `denied` and unknown names can only exit.
        for p in ["denied", "whatever", ""] {
            for n in all {
                assert_eq!(permits(allow_mask(p), n), n == Number::Exit, "{p} {n:?}");
            }
        }
    }

    #[test]
    fn permits_raw_legacy_and_linux() {
        let mask = allow_mask("hello");
        // Legacy exit (n=2, a0=0)
        assert!(permits_raw(mask, 2, 0));
        // Linux open (n=2, a0=0x4000) not permitted for hello
        assert!(!permits_raw(mask, 2, 0x4000));
        // Linux exit (n=60)
        assert!(permits_raw(mask, 60, 0));
        // Linux write (n=1)
        assert!(permits_raw(mask, 1, 1));

        let denied_mask = allow_mask("denied");
        assert!(permits_raw(denied_mask, 60, 0));
        assert!(permits_raw(denied_mask, 2, 0));
        assert!(!permits_raw(denied_mask, 4, 0)); // uptime denied
    }

    #[test]
    fn number_bits_are_distinct() {
        let all = [
            Number::Read,
            Number::Write,
            Number::Open,
            Number::Close,
            Number::Lseek,
            Number::Mmap,
            Number::Brk,
            Number::Yield,
            Number::GetPid,
            Number::Exit,
            Number::ClockGetTime,
        ];
        let mut seen = 0u64;
        for n in all {
            assert_eq!(seen & n.bit(), 0, "duplicate bit for {n:?}");
            seen |= n.bit();
        }
        assert_eq!(seen & LEGACY_EXIT_BIT, 0);
        assert_eq!(seen & LEGACY_YIELD_BIT, 0);
    }

    #[test]
    fn decode_unknown_is_none() {
        for n in [4, 5, 6, 7, 10, 11, 13, 23, 25, 38, 40, 59, 61, 227, 229, u64::MAX] {
            assert_eq!(decode(n), None);
        }
    }

    #[test]
    fn sanitize_keeps_text_and_replaces_controls() {
        let mut b = *b"ok\tline\n";
        sanitize_output(&mut b);
        assert_eq!(&b, b"ok\tline\n");
        let mut c = *b"a\rb\x1b[2Jc\x00\x7f\xffd";
        sanitize_output(&mut c);
        assert_eq!(&c, b"a?b?[2Jc???d");
    }

    #[test]
    fn errno_values() {
        assert_eq!(errno(EPERM), u64::MAX);
        assert_eq!(errno(ENOENT), u64::MAX - 1);
        assert_eq!(errno(EBADF), u64::MAX - 8);
    }
}
