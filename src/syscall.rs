//! System call numbers. Pure, host-testable. Calling convention: number in `rax`, arguments in `rdi`,
//! `rsi`, `rdx`, result in `rax`, issued with the `syscall` instruction.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Number {
    /// `write(ptr, len)`: print `len` bytes at `ptr` (must be in the user window, at most `MAX_WRITE`).
    Write = 1,
    /// `exit(code)`
    Exit = 2,
    Yield = 3,
    /// microseconds since boot
    Uptime = 4,
    /// the calling task's id
    GetPid = 5,
}

pub const MAX_WRITE: u64 = 256;
/// Returned for bad pointers, bad lengths and unknown calls.
pub const ERR: u64 = u64::MAX;

pub fn decode(n: u64) -> Option<Number> {
    Some(match n {
        1 => Number::Write,
        2 => Number::Exit,
        3 => Number::Yield,
        4 => Number::Uptime,
        5 => Number::GetPid,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_roundtrip() {
        for n in [Number::Write, Number::Exit, Number::Yield, Number::Uptime, Number::GetPid] {
            assert_eq!(decode(n as u64), Some(n));
        }
    }

    #[test]
    fn decode_unknown_is_none() {
        for n in [0, 6, 255, u64::MAX] {
            assert_eq!(decode(n), None);
        }
    }
}
