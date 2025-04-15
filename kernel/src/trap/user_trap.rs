//! Trap from user.

use alloc::sync::Arc;

use arch::{
    interrupts::disable_interrupt,
    memory::VirtAddr,
    systype::SysError,
    trap::{self, TrapType},
};
use async_utils::yield_now;
use signal::{Sig, SigDetails, SigInfo};

use super::TrapContext;
use crate::{mm::PageFaultAccessType, syscall::Syscall, task::Task, trap::set_user_trap};

/// handle an interrupt, exception, or system call from user space
/// return if it is syscall and has been interrupted
#[unsafe(no_mangle)]
pub async fn trap_handler(task: &Arc<Task>) -> bool {
    let cx = task.trap_context_mut();
    let trap_type = trap::user_trap::trap_handler(cx);

    if task.time_stat_ref().need_schedule() && executor::has_task() {
        log::info!("time slice used up, yield now");
        yield_now().await;
    }

    match trap_type {
        TrapType::Breakpoint => {}
        TrapType::SysCall(syscall_no) => {
            // get system call return value
            let ret = Syscall::new(task)
                .syscall(syscall_no, cx.syscall_args())
                .await;
            cx.save_last_user_a0();
            cx.set_user_a0(ret);
            if ret == -(SysError::EINTR as isize) as usize {
                return true;
            }
        }
        TrapType::LoadPageFault(stval, sepc)
        | TrapType::StorePageFault(stval, sepc)
        | TrapType::InstructionPageFault(stval, sepc) => {
            log::info!(
                // "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x}
                // scause {cause:?}",
                "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} ",
            );
            let access_type = match trap_type {
                TrapType::InstructionPageFault(v, sepc) => PageFaultAccessType::RX,
                TrapType::LoadPageFault(v, sepc) => PageFaultAccessType::RO,
                TrapType::StorePageFault(v, sepc) => PageFaultAccessType::RW,
                _ => unreachable!(),
            };
            // There are serveral kinds of page faults:
            // 1. mmap area
            // 2. sbrk area
            // 3. fork cow area
            // 4. user stack
            // 5. execve elf file
            // 6. dynamic link
            // 7. illegal page fault
            let result = task
                .with_mut_memory_space(|m| m.handle_page_fault(VirtAddr::from(stval), access_type));
            if let Err(_e) = result {
                log::warn!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x}",
                    // "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x}
                    // scause {cause:?}",
                );
                // backtrace::backtrace();
                log::warn!("{:x?}", task.trap_context_mut());
                // task.with_memory_space(|m| m.print_all());
                log::warn!("bad memory access, send SIGSEGV to task");
                task.receive_siginfo(
                    SigInfo {
                        sig: Sig::SIGSEGV,
                        code: SigInfo::KERNEL,
                        details: SigDetails::None,
                    },
                    false,
                );
            }
        }
        TrapType::IllegalInstruction => {
            task.set_terminated();
        }
        TrapType::Timer => {
            // NOTE: User may trap into kernel frequently. As a consequence, this timer are
            // likely not triggered in user mode but rather be triggered in supervisor mode,
            // which will cause user program running on the cpu for a quite long time.
            if executor::has_task() {
                yield_now().await;
            }
        }
        TrapType::SupervisorExternal => {
            log::info!("[kernel] receive externel interrupt");
            driver::get_device_manager_mut().handle_irq();
        }

        e => {
            log::warn!("Unknown user exception: {:?}", e);
        }
    }
    return false;
}

/// Trap return to user mode.
#[unsafe(no_mangle)]
pub fn trap_return(task: &Arc<Task>) {
    log::info!("[kernel] trap return to user...");
    unsafe {
        disable_interrupt();
        set_user_trap()
        // WARN: stvec can not be changed below. One hidden mistake is to use
        // `UserPtr` implicitly which will change stvec to `__trap_from_kernel`.
    };
    task.time_stat().record_trap_return();
    assert!(!task.is_terminated() && !task.is_zombie());

    trap::user_trap::trap_return(task.trap_context_mut());

    task.time_stat().record_trap();
}
