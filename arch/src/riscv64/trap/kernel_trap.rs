use riscv::{
    interrupt::{Exception, Trap, supervisor},
    register::{
        scause::{self, Scause},
        sepc, sstatus, stval, stvec,
    },
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
