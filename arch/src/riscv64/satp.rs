use riscv::register::satp::{self, Satp};

pub fn read_bits() -> usize {
    satp::read().bits()
}

pub fn write_bits(bits: usize) {
    let satp = Satp::from_bits(bits);
    unsafe { satp::write(satp) };
}
