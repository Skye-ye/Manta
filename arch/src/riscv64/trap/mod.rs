pub mod context;
pub mod kernel_trap;
pub mod user_trap;
use core::arch::global_asm;

pub use context::TrapContext;

use crate::interrupts::set_trap_handler;

global_asm!(include_str!("trap.asm"));

unsafe extern "C" {
    fn __trap_from_user();
    fn __trap_from_kernel();
}
pub fn init() {
    unsafe { set_kernel_trap() };
}

pub unsafe fn set_kernel_trap() {
    unsafe { set_trap_handler(__trap_from_kernel as usize) };
}

unsafe fn set_user_trap() {
    unsafe { set_trap_handler(__trap_from_user as usize) };
}
