#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(riscv_ext_intrinsics)]
#![feature(negative_impls)]
#![feature(sync_unsafe_cell)]

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(target_arch = "loongarch64")]
pub mod loongarch64;

#[cfg(target_arch = "loongarch64")]
pub use loongarch64::*;
