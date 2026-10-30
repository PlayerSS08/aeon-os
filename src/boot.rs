//! The kernel's single `BootInfo`, filled during boot and read-only afterwards.

use crate::bootinfo::BootInfo;
use crate::global::Global;

static BOOT: Global<BootInfo> = Global::new(BootInfo::new());

/// Access to the boot information.
///
/// Boot is single-threaded while this is being filled, and afterwards callers only read it, keeping
/// each reference short-lived; that is the invariant that makes handing out `&mut` here sound.
pub fn info() -> &'static mut BootInfo {
    // SAFETY: see the function comment.
    unsafe { BOOT.get() }
}
