#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(riscv_ext_intrinsics)]

extern crate alloc;

#[cfg(target_arch = "riscv64")]
mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;
