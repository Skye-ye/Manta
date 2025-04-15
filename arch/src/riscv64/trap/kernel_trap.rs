use riscv::{
    interrupt::{Exception, Trap, supervisor},
    register::{
        scause::{self, Scause},
        sepc, sstatus, stval, stvec,
    },
};

use super::TrapType;
use crate::{
    interrupts::set_trap_handler_vector, time::set_next_timer_irq, timer::TIMER_MANAGER, when_debug,
};
pub fn panic_on_unknown_trap() {
    panic!(
        "[kernel] sstatus sum {}, {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        sstatus::read().sum(),
        scause::read().cause(),
        scause::read().bits(),
        stval::read(),
        sepc::read(),
    );
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

pub fn kernel_trap_handler() -> TrapType {
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let trap = scause.cause();
    match trap.try_into() {
        Ok(Trap::Interrupt(i)) => match i {
            supervisor::Interrupt::SupervisorExternal => TrapType::SupervisorExternal,
            supervisor::Interrupt::SupervisorTimer => {
                // log::error!("[kernel_trap] receive timer interrupt");
                TIMER_MANAGER.check();
                unsafe { set_next_timer_irq() };
                TrapType::Timer
            }
            _ => TrapType::Unknown,
        },
        Ok(Trap::Exception(e)) => match e {
            Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault => {
                log::info!(
                    "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} cause {:?}",
                    e,
                );

                match e {
                    Exception::StorePageFault => TrapType::StorePageFault(stval, sepc),
                    Exception::InstructionPageFault => TrapType::InstructionPageFault(stval, sepc),
                    Exception::LoadPageFault => TrapType::LoadPageFault(stval, sepc),
                    _ => TrapType::Unknown,
                }
            }
            _ => TrapType::Unknown,
        },
        Err(_) => TrapType::Unknown,
    }
}
