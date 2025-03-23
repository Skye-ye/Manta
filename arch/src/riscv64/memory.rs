pub unsafe fn sfence_vma_vaddr(vaddr: usize) {
    unsafe {
        core::arch::riscv64::sfence_vma_vaddr(vaddr);
    }
}

pub unsafe fn sfence_vma_all() {
    unsafe {
        core::arch::riscv64::sfence_vma_all();
    }
}
