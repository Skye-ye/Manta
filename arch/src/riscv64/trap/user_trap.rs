use riscv::{
    interrupt::{Exception, Trap, supervisor},
    register::{scause, sepc, stval},
};

use super::{TrapContext, TrapType, set_kernel_trap};
use crate::{
    interrupts::enable_interrupt, sstatus, time::set_next_timer_irq, timer::TIMER_MANAGER,
};

// 内核中断回调
// #[unsafe(no_mangle)]
pub fn trap_handler(cx: &mut TrapContext) -> TrapType {
    unsafe { set_kernel_trap() };
    let scause = scause::read();
    let stval = stval::read();
    let sepc = sepc::read();
    let cause = scause.cause();
    log::trace!("[trap_handler] user task trap into kernel");
    log::trace!("[trap_handler] sepc:{sepc:#x}, stval:{stval:#x}");
    unsafe { enable_interrupt() };

    let trap_type = match cause.try_into() {
        Ok(Trap::Exception(e)) => match e {
            Exception::Breakpoint => {
                cx.sepc += 4;
                TrapType::Breakpoint
            }
            Exception::UserEnvCall => {
                let syscall_no = cx.syscall_no();
                cx.set_user_pc_to_next();
                TrapType::SysCall(syscall_no)
            }
            Exception::StorePageFault => TrapType::StorePageFault(stval, sepc),
            Exception::InstructionPageFault => TrapType::InstructionPageFault(stval, sepc),
            Exception::LoadPageFault => TrapType::LoadPageFault(stval, sepc),
            Exception::IllegalInstruction => {
                log::warn!(
                    "[trap_handler] detected illegal instruction, stval {stval:#x}, sepc {sepc:#x}",
                );
                TrapType::IllegalInstruction
            }
            e => {
                log::warn!("Unknown user exception: {:?}", e);
                TrapType::Unknown
            }
        },

        Ok(Trap::Interrupt(i)) => {
            match i {
                supervisor::Interrupt::SupervisorTimer => {
                    // NOTE: User may trap into kernel frequently. As a consequence, this timer are
                    // likely not triggered in user mode but rather be triggered in supervisor mode,
                    // which will cause user program running on the cpu for a quite long time.
                    log::trace!("[trap_handler] timer interrupt, sepc {sepc:#x}");
                    TIMER_MANAGER.check();
                    unsafe { set_next_timer_irq() };
                    TrapType::Timer
                }
                supervisor::Interrupt::SupervisorExternal => TrapType::SupervisorExternal,
                _ => {
                    panic!(
                        "[trap_handler] Unsupported trap {cause:?}, stval = {stval:#x}!, sepc = {sepc:#x}"
                    );
                }
            }
        }
        Err(_) => {
            panic!(
                "[trap_handler] Error when converting trap to target-specific trap cause {:?}",
                cause
            );
        }
    };
    trap_type
}
unsafe extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

pub fn trap_return(cx: &mut TrapContext) {
    // 2. This task encounter a signal handler
    cx.user_fx.restore();
    sstatus::set_fs_clean(cx.sstatus);
    assert!(!cx.sstatus.sie());
    unsafe {
        __return_to_user(cx);
        // NOTE: next time when user traps into kernel, it will come back here
        // and return to `user_loop` function.
    }

    let need_save = sstatus::is_fs_dirty(cx.sstatus) as u8;

    cx.user_fx.mark_save_if_needed(need_save);
}
