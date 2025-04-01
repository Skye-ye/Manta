use core::{
    cell::UnsafeCell,
    fmt,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr,
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

unsafe impl<T: Send> Send for StaticCell<T> {}
unsafe impl<T: Send + Sync> Sync for StaticCell<T> {}

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
        if self.initialized.swap(true, Ordering::AcqRel) {
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
    #[inline]
    pub fn get(&self) -> &T {
        if !self.initialized.load(Ordering::Acquire) {
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
    #[inline]
    pub fn get_mut(&self) -> &mut T {
        if !self.initialized.load(Ordering::Acquire) {
            panic!("StaticCell not initialized");
        }

        // SAFETY: We've verified the cell is initialized
        unsafe { &mut *(*self.value.get()).as_mut_ptr() }
    }
}

impl<T: fmt::Debug> fmt::Debug for StaticCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.initialized.load(Ordering::Acquire) {
            write!(
                f,
                "StaticCell {{ initialized: true, value: {:?} }}",
                self.get()
            )
        } else {
            write!(f, "StaticCell {{ initialized: false }}")
        }
    }
}

impl<T> Deref for StaticCell<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> DerefMut for StaticCell<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T> Drop for StaticCell<T> {
    fn drop(&mut self) {
        if self.initialized.load(Ordering::Relaxed) {
            unsafe {
                ptr::drop_in_place(self.value.get());
            }
        }
    }
}
