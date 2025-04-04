#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(riscv_ext_intrinsics)]

#[cfg(feature = "riscv64")]
mod riscv64;

#[cfg(feature = "loongarch")]
mod loongarch64;

#[cfg(feature = "loongarch")]
pub use loongarch64::*;
#[cfg(feature = "riscv64")]
pub use riscv64::*;
