pub use config::mm::{PAGE_MASK, PAGE_SIZE, PAGE_SIZE_BITS};

// ==================== 物理内存配置 ====================
#[cfg(not(feature = "vf2"))]
pub const RAM_START: usize = 0x8000_0000;
#[cfg(feature = "vf2")]
pub const RAM_START: usize = 0x8000_0000;

#[cfg(not(feature = "vf2"))]
pub const RAM_SIZE: usize = 128 * 1024 * 1024; // 128 MiB
#[cfg(feature = "vf2")]
pub const RAM_SIZE: usize = 0x100000000 + RAM_START - 0x4000_0000;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const MEMORY_END: usize = VIRT_START + RAM_SIZE;

// ==================== 虚拟内存映射 ====================
pub const HIGH_HALF: usize = 0xffff_ffc0_0000_0000;
pub const VIRT_START: usize = HIGH_HALF + RAM_START;
pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

// ==================== 内核空间布局 ====================
pub const K_SEG_BEG: usize = 0xffff_ffc0_0000_0000;

// 内核堆空间 (64GiB)
pub const K_SEG_HEAP_BEG: usize = 0xffff_ffc0_0000_0000;
pub const K_SEG_HEAP_END: usize = 0xffff_ffd0_0000_0000;

// 文件映射区 (64GiB)
pub const K_SEG_FILE_BEG: usize = 0xffff_ffd0_0000_0000;
pub const K_SEG_FILE_END: usize = 0xffff_ffe0_0000_0000;

// 物理内存映射区 (62GiB)
pub const K_SEG_PHY_MEM_BEG: usize = 0xffff_fff0_0000_0000;
pub const K_SEG_PHY_MEM_END: usize = 0xffff_ffff_8000_0000;

// 内核代码段 (1GiB)
pub const K_SEG_TEXT_BEG: usize = 0xffff_ffff_8000_0000;
pub const K_SEG_TEXT_END: usize = 0xffff_ffff_c000_0000;

// 设备树固定映射区
pub const K_SEG_DTB_END: usize = 0xffff_ffff_f000_0000;
pub const MAX_DTB_SIZE: usize = PAGE_SIZE * PAGE_SIZE;
pub const K_SEG_DTB_BEG: usize = K_SEG_DTB_END - MAX_DTB_SIZE;

// ==================== 用户空间布局 ====================
// 用户栈空间 (256MiB)
pub const U_SEG_STACK_BEG: usize = 0x0000_0001_0000_0000;
pub const U_SEG_STACK_END: usize = 0x0000_0002_0000_0000;

// 用户堆空间 (64MiB)
pub const U_SEG_HEAP_BEG: usize = 0x0000_0000_4000_0000;
pub const U_SEG_HEAP_END: usize = 0x0000_0000_8000_0000;

// 用户文件映射区 (512MiB)
pub const U_SEG_FILE_BEG: usize = 0x0000_0004_0000_0000;
pub const U_SEG_FILE_END: usize = 0x0000_0006_0000_0000;

// 共享内存区 (512MiB)
pub const U_SEG_SHARE_BEG: usize = 0x0000_0006_0000_0000;
pub const U_SEG_SHARE_END: usize = 0x0000_0008_0000_0000;

// 动态链接器加载偏移
pub const DL_INTERP_OFFSET: usize = 0x20_0000_0000;

// ==================== 页表配置 ====================
pub const PTE_SIZE: usize = 8;
pub const PTES_PER_PAGE: usize = PAGE_SIZE / PTE_SIZE;
pub const PAGE_TABLE_LEVEL_NUM: usize = 3;

// ==================== 多核启动配置 ====================
pub const HART_START_ADDR: usize = RAM_START + KERNEL_START;
