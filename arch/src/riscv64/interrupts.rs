use riscv::register::{
    sie, sstatus,
    stvec::{self, TrapMode},
};

pub fn is_interrupt_enabled() -> bool {
    sstatus::read().sie()
}

pub unsafe fn enable_interrupt() {
    #[cfg(feature = "irq")]
    unsafe {
        sstatus::set_sie();
    }
}

pub unsafe fn disable_interrupt() {
    #[cfg(feature = "irq")]
    unsafe {
        sstatus::clear_sie();
    }
}

pub unsafe fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

pub unsafe fn enable_external_interrupt() {
    unsafe {
        sie::set_sext();
    }
}

pub fn get_trap_handler() -> usize {
    stvec::read().bits()
}

pub unsafe fn set_trap_handler(handler_addr: usize) {
    unsafe {
        stvec::write(handler_addr, TrapMode::Direct);
    }
}

pub unsafe fn set_trap_handler_vector(handler_addr: usize) {
    unsafe {
        stvec::write(handler_addr, TrapMode::Vectored);
    }
}

/// Disable interrupt and resume to the interrupt state before when it gets
/// dropped.
pub struct InterruptGuard {
    interrupt_before: bool,
}

impl InterruptGuard {
    pub fn new() -> Self {
        let interrupt_before = is_interrupt_enabled();
        unsafe { disable_interrupt() };
        Self { interrupt_before }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.interrupt_before {
            unsafe { enable_interrupt() };
        }
    }
}

pub struct TrapHandlerGuard {
    trap_handler_before: usize,
}

impl TrapHandlerGuard {
    pub fn new(new_trap_handler: usize) -> Self {
        let trap_handler_before = get_trap_handler();
        unsafe { set_trap_handler(new_trap_handler) }
        Self {
            trap_handler_before,
        }
    }
}

impl Drop for TrapHandlerGuard {
    fn drop(&mut self) {
        unsafe { set_trap_handler(self.trap_handler_before) }
    }
}
