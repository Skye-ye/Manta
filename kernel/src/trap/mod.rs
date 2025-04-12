//! Trap handling functionality

pub mod context;
pub mod kernel_trap;
pub mod user_trap;

pub use arch::trap::{init, set_kernel_trap, set_user_trap};
pub use context::TrapContext;
