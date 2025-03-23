use riscv::register::sstatus::{self, Sstatus};

pub fn read_bits() -> usize {
    sstatus::read().bits()
}

pub fn write_bits(bits: usize) {
    let sstatus = Sstatus::from_bits(bits);
    unsafe { sstatus::write(sstatus) };
}
