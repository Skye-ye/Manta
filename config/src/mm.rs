//! 内存管理相关配置和工具函数

use crate::{
    board::{BLOCK_MASK, BLOCK_SIZE},
    utils::register_mut_const,
};

// =============================================
// 内存管理核心常量
// =============================================

/// 页大小配置 (4KB 标准页)
pub const PAGE_SIZE_BITS: usize = 12; // 页大小位数
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS; // 4096 bytes
pub const PAGE_MASK: usize = PAGE_SIZE - 1; // 页对齐掩码

/// 内核栈配置
pub const KERNEL_STACK_SIZE: usize = 64 * 1024; // 64KB 内核栈

/// 内核堆配置（按开发板区分）
#[cfg(not(feature = "vf2"))]
pub const KERNEL_HEAP_SIZE: usize = 64 * 1024 * 1024; // 64MB 默认堆
#[cfg(feature = "vf2")]
pub const KERNEL_HEAP_SIZE: usize = 256 * 1024 * 1024; // 256MB VisionFive2 专用

// =============================================
// 设备树配置
// =============================================
register_mut_const!(pub DTB_ADDR, usize, 0); // 动态设备树地址寄存器

// =============================================
// 用户空间预分配配置
// =============================================
pub const USER_ELF_PRE_ALLOC_PAGE_CNT: usize = 0; // ELF预分配页数
pub const MMAP_PRE_ALLOC_PAGES: usize = 8; // 内存映射预分配页

// =============================================
// 块设备缓存配置
// =============================================
pub const MAX_BUFFER_HEADS: usize = 0x18000; // 最大缓存头数量
pub const MAX_BUFFER_CACHE: usize = 0x1000; // 最大缓存大小
pub const MAX_BUFFERS_PER_PAGE: usize = PAGE_SIZE / BLOCK_SIZE; // 每页块数
pub const MAX_BUFFER_PAGES: usize = MAX_BUFFER_CACHE / MAX_BUFFERS_PER_PAGE; // 缓存页数
pub const BUFFER_NEED_CACHE_CNT: usize = 8; // 缓存需求阈值

// =============================================
// 内存操作工具函数
// =============================================

/// 页对齐计算工具
pub fn align_offset_to_page(offset: usize) -> (usize, usize) {
    let aligned = offset & !PAGE_MASK;
    (aligned, offset - aligned)
}

/// 页对齐检查
pub fn is_aligned_to_page(offset: usize) -> bool {
    offset & PAGE_MASK == 0
}

/// 块对齐检查
pub fn is_aligned_to_block(offset: usize) -> bool {
    offset & BLOCK_MASK == 0
}

/// 向下页对齐
pub fn round_down_to_page(offset: usize) -> usize {
    offset & !PAGE_MASK
}

/// 向上页对齐
pub fn round_up_to_page(offset: usize) -> usize {
    (offset + PAGE_MASK) & !PAGE_MASK
}

/// 计算块所属页号
pub fn block_page_id(block_id: usize) -> usize {
    block_id / MAX_BUFFERS_PER_PAGE
}

/// 计算块在页内的偏移量
pub fn block_page_offset(block_id: usize) -> usize {
    (block_id % MAX_BUFFERS_PER_PAGE) * BLOCK_SIZE
}

#[cfg(feature = "riscv64")]
#[cfg(not(feature = "vf2"))]
pub const RAM_SIZE: usize = 128 * 1024 * 1024; // 128 MiB
#[cfg(feature = "riscv64")]
#[cfg(feature = "vf2")]
pub const RAM_SIZE: usize = 0x100000000 + RAM_START - 0x4000_0000;

#[cfg(feature = "riscv64")]
pub const KERNEL_OFFSET: usize = 0x20_0000; // 内核物理偏移 2MB

#[cfg(feature = "riscv64")]
pub const RAM_START: usize = 0x8000_0000;

#[cfg(feature = "riscv64")]
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;

#[cfg(feature = "riscv64")]
pub const KERNEL_START: usize = 0xffff_ffc0_8020_0000;
