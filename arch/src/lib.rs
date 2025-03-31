#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(riscv_ext_intrinsics)]
#![feature(negative_impls)]
#![feature(sync_unsafe_cell)]
#![feature(const_trait_impl)]
#![feature(step_trait)]
#![feature(const_ops)]

extern crate alloc;

#[cfg(target_arch = "riscv64")]
mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;
