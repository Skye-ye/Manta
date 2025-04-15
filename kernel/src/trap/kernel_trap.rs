//! Trap from kernel.

use arch::{
    memory::VirtAddr,
    trap::{TrapType, kernel_trap::panic_on_unknown_trap},
};
use signal::{Sig, SigDetails, SigInfo};

use crate::{mm::PageFaultAccessType, processor::hart::current_task_ref};

/// Kernel trap handler
#[unsafe(no_mangle)]
pub fn kernel_trap_handler() {
    let trap_type = arch::trap::kernel_trap::kernel_trap_handler();
    match trap_type {
        TrapType::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
            driver::get_device_manager_mut().handle_irq();
        }
        TrapType::Timer => {
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
        TrapType::LoadPageFault(stval, sepc)
        | TrapType::StorePageFault(stval, sepc)
        | TrapType::InstructionPageFault(stval, sepc) => {
            let access_type = match trap_type {
                TrapType::InstructionPageFault(stval, sepc) => PageFaultAccessType::RX,
                TrapType::LoadPageFault(stval, sepc) => PageFaultAccessType::RO,
                TrapType::StorePageFault(stval, sepc) => PageFaultAccessType::RW,
                _ => unreachable!(),
            };
            let result = current_task_ref()
                .with_mut_memory_space(|m| m.handle_page_fault(VirtAddr::from(stval), access_type));
            if let Err(_e) = result {
                log::warn!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} cause {:?}",
                    trap_type,
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
    }
}
