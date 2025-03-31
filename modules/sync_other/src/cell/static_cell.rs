use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

/// A thread-safe cell that can be initialized exactly once.
///
/// This is a more efficient alternative to using `static mut` with unsafe code.
/// It uses MaybeUninit to avoid Option overhead and uses AtomicBool to track
/// initialization.
pub struct StaticCell<T> {
    initialized: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Sync for StaticCell<T> {}

impl<T> StaticCell<T> {
    /// Creates a new uninitialized static cell.
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Initializes the cell with a value.
    ///
    /// # Panics
    ///
    /// Panics if the cell is already initialized.
    pub fn init(&self, value: T) {
        // Ensure the cell hasn't been initialized already
        if self.initialized.swap(true, Ordering::SeqCst) {
            panic!("StaticCell already initialized");
        }

        // SAFETY: We've ensured this is only called once via the atomic flag
        unsafe {
            (*self.value.get()).write(value);
        }
    }

    /// Gets a reference to the value.
    ///
    /// # Panics
    ///
    /// Panics if the cell has not been initialized.
    pub fn get(&self) -> &T {
        if !self.initialized.load(Ordering::SeqCst) {
            panic!("StaticCell not initialized");
        }

        // SAFETY: We've verified the cell is initialized
        unsafe { &*(*self.value.get()).as_ptr() }
    }

    /// Gets a mutable reference to the value.
    ///
    /// # Panics
    ///
    /// Panics if the cell has not been initialized.
    pub fn get_mut(&self) -> &mut T {
        if !self.initialized.load(Ordering::SeqCst) {
            panic!("StaticCell not initialized");
        }

        // SAFETY: We've verified the cell is initialized
        unsafe { &mut *(*self.value.get()).as_mut_ptr() }
    }
}
