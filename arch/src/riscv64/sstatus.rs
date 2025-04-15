use riscv::register::sstatus::{self, FS, Sstatus};

pub fn read_bits() -> usize {
    sstatus::read().bits()
}

pub fn write_bits(bits: usize) {
    let sstatus = Sstatus::from_bits(bits);
    unsafe { sstatus::write(sstatus) };
}

pub fn set_fs_initial() {
    unsafe {
        sstatus::set_fs(FS::Initial);
    }
}

pub fn set_fs_clean(mut sstatus: Sstatus) {
    sstatus.set_fs(FS::Clean);
}

pub fn is_fs_dirty(sstatus: Sstatus) -> bool {
    sstatus.fs() == FS::Dirty
}
