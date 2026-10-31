//! Validation of pointers that user code hands to the kernel. Pure, host-testable.

/// The one window of memory user programs may touch: 1 MiB at 1 GiB.
pub const USER_BASE: u64 = 0x4000_0000;
pub const USER_END: u64 = 0x4010_0000;

/// True when `[ptr, ptr + len)` lies entirely inside the user window. A zero length is fine as long as
/// the pointer itself is inside or at the end of the window.
pub fn check(ptr: u64, len: u64) -> bool {
    match ptr.checked_add(len) {
        Some(end) => ptr >= USER_BASE && end <= USER_END,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overflow() {
        assert!(!check(u64::MAX - 1, 8));
        assert!(!check(USER_BASE, u64::MAX));
    }

    #[test]
    fn rejects_kernel_range() {
        assert!(!check(0, 1));
        assert!(!check(0x10_0000, 16));
        assert!(!check(0xFFFF_8000_0000_0000, 1));
        assert!(!check(USER_BASE - 1, 1));
    }

    #[test]
    fn rejects_straddle() {
        assert!(!check(USER_BASE - 4, 8));
        assert!(!check(USER_END - 4, 8));
    }

    #[test]
    fn accepts_zero_len_inside() {
        assert!(check(USER_BASE, 0));
        assert!(check(USER_END, 0));
        assert!(!check(USER_END + 1, 0));
        assert!(!check(0, 0));
    }

    #[test]
    fn accepts_exact_window() {
        assert!(check(USER_BASE, USER_END - USER_BASE));
        assert!(!check(USER_BASE, USER_END - USER_BASE + 1));
    }
}
