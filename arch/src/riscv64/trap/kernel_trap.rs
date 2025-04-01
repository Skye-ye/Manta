//! Trap from kernel.

use arch::{interrupts::set_trap_handler_vector, memory::VirtAddr, time::set_next_timer_irq};
use riscv::{
    interrupt::{Exception, Trap, supervisor},
    register::{
        scause::{self, Scause},
        sepc, sstatus, stval, stvec,
    },
};
use signal::{Sig, SigDetails, SigInfo};
use arch::timer::TIMER_MANAGER;

use crate::{
    mm::PageFaultAccessType,
    processor::hart::{
        current_task_ref, local_hart, local_hart_disable_preemptable,
        local_hart_enable_preemptable, local_hart_preemptable,
    },
    when_debug,
};

fn panic_on_unknown_trap() {
    panic!(
        "[kernel] sstatus sum {}, {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        sstatus::read().sum(),
        scause::read().cause(),
        scause::read().bits(),
        stval::read(),
        sepc::read(),
    );
}

/// Kernel trap handler
#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let trap = scause.cause();

    match trap.try_into() {
        Ok(Trap::Interrupt(i)) => match i {
            supervisor::Interrupt::SupervisorExternal => {
                log::info!("[kernel] receive externel interrupt");
                driver::get_device_manager_mut().handle_irq();
            }
            supervisor::Interrupt::SupervisorTimer => {
                // log::error!("[kernel_trap] receive timer interrupt");
                TIMER_MANAGER.check();
                unsafe { set_next_timer_irq() };
                #[cfg(feature = "preempt")]
                {
                    use crate::processor::hart::local_hart;

                    if !executor::has_prior_task() {
                        return;
                    } else if !local_hart_preemptable() {
                        return;
                    }
                    local_hart_disable_preemptable();
                    // log::error!("env {:?}", local_hart().env());
                    let mut old_hart = local_hart().enter_preempt_switch();
                    // log::error!("kernel preempt");
                    executor::run_prior_until_idle();
                    // log::error!("kernel preempt fininshed");
                    local_hart().leave_preempt_switch(&mut old_hart);
                    local_hart_enable_preemptable();
                }
            }
            _ => panic_on_unknown_trap(),
        },
        Ok(Trap::Exception(e)) => match e {
            Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault => {
                log::info!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} cause {:?}",
                    e,
                );
                let access_type = match e {
                    Exception::InstructionPageFault => PageFaultAccessType::RX,
                    Exception::LoadPageFault => PageFaultAccessType::RO,
                    Exception::StorePageFault => PageFaultAccessType::RW,
                    _ => unreachable!(),
                };
                let result = current_task_ref().with_mut_memory_space(|m| {
                    m.handle_page_fault(VirtAddr::from(stval), access_type)
                });
                if let Err(_e) = result {
                    log::warn!(
                        "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} cause {:?}",
                        e,
                    );
                    log::warn!("{:x?}", current_task_ref().trap_context_mut());
                    log::warn!("bad memory access, send SIGSEGV to task");
                    current_task_ref().receive_siginfo(
                        SigInfo {
                            sig: Sig::SIGSEGV,
                            code: SigInfo::KERNEL,
                            details: SigDetails::None,
                        },
                        false,
                    );
                }
            }
            _ => panic_on_unknown_trap(),
        },
        Err(_) => panic_on_unknown_trap(),
    }
}

unsafe extern "C" {
    fn __user_rw_trap_vector();
}

pub unsafe fn set_kernel_user_rw_trap() {
    let trap_vaddr = __user_rw_trap_vector as usize;
    unsafe { set_trap_handler_vector(trap_vaddr) };
    log::trace!("[user check] switch to user rw checking mode at stvec: {trap_vaddr:#x}",);
}

pub fn will_read_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert_eq!(curr_stvec, __user_rw_trap_vector as usize);
    });

    unsafe extern "C" {
        fn __try_read_user(ptr: usize) -> TryOpRet;
    }
    let try_op_ret = unsafe { __try_read_user(vaddr) };
    match try_op_ret.flag() {
        0 => false,
        _ => {
            when_debug!({
                let scause: Scause = try_op_ret.scause();
                let raw_trap = scause.cause();
                if let Ok(trap) =
                    raw_trap.try_into::<supervisor::Interrupt, supervisor::Exception>()
                {
                    match trap {
                        Trap::Interrupt(_) => panic!("Unexpected interrupt during read check"),
                        Trap::Exception(e) => assert_eq!(e, supervisor::Exception::LoadPageFault),
                    }
                }
            });
            true
        }
    }
}

pub fn will_write_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert!(curr_stvec == __user_rw_trap_vector as usize);
    });
    unsafe extern "C" {
        fn __try_write_user(vaddr: usize) -> TryOpRet;
    }
    let try_op_ret = unsafe { __try_write_user(vaddr) };
    match try_op_ret.flag() {
        0 => false,
        _ => {
            when_debug!({
                let scause: Scause = try_op_ret.scause();
                let raw_trap = scause.cause();
                if let Ok(trap) =
                    raw_trap.try_into::<supervisor::Interrupt, supervisor::Exception>()
                {
                    match trap {
                        Trap::Interrupt(_) => panic!("Unexpected interrupt during write check"),
                        Trap::Exception(e) => assert_eq!(e, supervisor::Exception::StorePageFault),
                    }
                }
            });
            true
        }
    }
}

#[repr(C)]
struct TryOpRet {
    flag: usize,
    scause: usize,
}

impl TryOpRet {
    pub fn flag(&self) -> usize {
        self.flag
    }

    pub fn scause(&self) -> Scause {
        unsafe { core::mem::transmute(self.scause) }
    }
}
