use core::time::Duration;

use config::time::INTERRUPTS_PER_SECOND;
use riscv::register::time;

use crate::config::board::clock_freq;

pub mod stat;
pub mod timespec;
pub mod timeval;
pub mod tms;

// clockid
pub const SUPPORT_CLOCK: usize = 2;
/// 一个可设置的系统级实时时钟，用于测量真实（即墙上时钟）时间
pub const CLOCK_REALTIME: usize = 0;
/// 一个不可设置的系统级时钟，代表自某个未指定的过去时间点以来的单调时间
pub const CLOCK_MONOTONIC: usize = 1;
/// 用于测量调用进程消耗的CPU时间
pub const CLOCK_PROCESS_CPUTIME_ID: usize = 2;
/// 用于测量调用线程消耗的CPU时间
pub const CLOCK_THREAD_CPUTIME_ID: usize = 3;

pub static mut CLOCK_DEVIATION: [Duration; SUPPORT_CLOCK] = [Duration::ZERO; SUPPORT_CLOCK];

pub fn get_time() -> usize {
    time::read()
}

/// milliseconds 毫秒
pub fn get_time_ms() -> usize {
    time::read() / (clock_freq() / 1_000)
}

pub fn get_time_sec() -> usize {
    time::read() / clock_freq()
}

/// microseconds 微秒
pub fn get_time_us() -> usize {
    time::read() / (clock_freq() / 1_000_000)
}

pub fn get_time_duration() -> Duration {
    Duration::from_micros(get_time_us() as u64)
}

pub unsafe fn set_next_timer_irq() {
    let next_trigger: u64 = (time::read() + clock_freq() / INTERRUPTS_PER_SECOND) as u64;
    sbi_rt::set_timer(next_trigger);
}

pub unsafe fn set_timer_irq(times: usize) {
    let next_trigger: u64 = (time::read() + times * clock_freq() / INTERRUPTS_PER_SECOND) as u64;
    sbi_rt::set_timer(next_trigger);
}
