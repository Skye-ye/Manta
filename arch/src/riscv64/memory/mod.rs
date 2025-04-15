pub mod address;
pub mod frame;
pub mod heap;
pub mod page_table;
pub mod pte;

pub use address::*;
pub use frame::*;
pub use page_table::PageTable;
pub use pte::PageTableEntry;

pub struct TLB;

impl TLB {
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
}
