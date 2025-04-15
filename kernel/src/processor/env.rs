use super::hart::local_hart;

/// use RAII to guard `sum` flag.
pub struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        local_hart().env_mut().inc_sum();
        Self
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        local_hart().env_mut().dec_sum();
    }
}

pub fn within_sum<T>(f: impl FnOnce() -> T) -> T {
    let _guard = SumGuard::new();
    let ret = f();
    ret
}
