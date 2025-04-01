pub mod entry;
pub mod interrupts;
#[cfg(feature = "kernel")]
pub mod memory;
pub mod register;
pub mod satp;
pub mod sstatus;
pub mod sync;
pub mod systype;
pub mod time;

pub use entry::*;
pub use interrupts::*;
pub use memory::*;
pub use register::*;
pub use satp::*;
pub use sstatus::*;
pub use time::*;

pub mod timer;
// pub mod trap;

#[inline(never)]
pub fn spin(cycle: usize) {
    for _ in 0..cycle {
        core::hint::spin_loop();
    }
}
