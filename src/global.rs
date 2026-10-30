//! Minimal interior-mutability cell for kernel globals.
//!
//! Aeon runs on one CPU with interrupts disabled except for a handler that only touches an atomic
//! tick counter, so a `static` guarded by "only one thread of control exists" is sound. Each use site
//! documents why it upholds that.

use core::cell::UnsafeCell;

pub struct Global<T>(UnsafeCell<T>);

// SAFETY: there is a single hardware thread and no shared access from interrupt context (see the
// module comment), so no two threads of control can reach the same `Global` concurrently.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    /// Raw pointer to the contents.
    pub fn as_ptr(&self) -> *mut T {
        self.0.get()
    }

    /// Mutable access to the contents.
    ///
    /// # Safety
    /// The caller must ensure no other reference to the contents is live, i.e. this is not called
    /// re-entrantly while a previous `get` result is still in use.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get(&self) -> &mut T {
        // SAFETY: exclusivity is the caller's obligation, stated in the function contract.
        unsafe { &mut *self.0.get() }
    }
}
