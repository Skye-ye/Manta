//! Boot process is adapted from Titanix

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(core_intrinsics)]
#![feature(let_chains)]
#![feature(const_trait_impl)]
#![feature(sync_unsafe_cell)]
#![feature(riscv_ext_intrinsics)]
#![feature(map_try_insert)]
#![feature(new_zeroed_alloc)]
#![allow(clippy::mut_from_ref)]

mod boot;
mod impls;
mod ipc;
mod mm;
mod net;
mod panic;
mod processor;
mod syscall;
mod task;
mod trap;
mod utils;
use core::{
    arch::global_asm,
    sync::atomic::{AtomicBool, Ordering},
};

use ::net::poll_interfaces;

use crate::processor::hart;

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate driver;

#[macro_use]
extern crate logging;

global_asm!(include_str!("trampoline.asm"));

static FIRST_HART: AtomicBool = AtomicBool::new(true);

#[unsafe(no_mangle)]
fn rust_main(hart_id: usize, dtb_addr: usize) {
    if FIRST_HART
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        boot::clear_bss();
        boot::print_banner();

        hart::init(hart_id);
        logging::init();

        println!("[kernel] ---------- main hart {hart_id} started ---------- ");
        config::mm::set_dtb_addr(dtb_addr);

        mm::init();
        trap::init();
        driver::init();
        vfs::init();

        task::spawn_kernel_task(async move {
            task::spawn_init_proc();
        });

        // utils::spawn_timer_tasks_ms(
        //     || {
        //         poll_interfaces();
        //     },
        //     10,
        // );

        #[cfg(feature = "debug")]
        utils::spawn_timer_tasks(utils::print_proc_tree, 10);

        #[cfg(feature = "smp")]
        boot::start_other_harts(hart_id);
    } else {
        hart::init(hart_id);
        trap::init();
        unsafe { mm::switch_kernel_page_table() };
    }

    unsafe {
        arch::interrupts::enable_timer_interrupt();
        arch::time::set_next_timer_irq()
    };

    println!("[kernel] ---------- hart {hart_id} start to fetch task... ---------- ");
    let mut try_count = 0usize;
    loop {
        let tasks = executor::run_until_idle();
        if tasks == 0 {
            try_count += 1;
        } else {
            try_count = 0;
        }
        if try_count >= 0x10000000 {
            panic!("no tasks")
        }
    }
}
