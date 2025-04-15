//! Trap handling functionality

// pub mod context;
pub mod kernel_trap;
pub mod user_trap;

pub use arch::trap::{
    context, init,
    kernel_trap::{set_kernel_user_rw_trap, will_read_fail, will_write_fail},
    set_kernel_trap, set_user_trap,
};
pub use context::TrapContext;
