//! Impls of traits defined in other crates.

use alloc::{fmt, string::ToString, sync::Arc};

use config::mm::VIRT_RAM_OFFSET;
use driver::KernelPageTableIf;
use log::Level;
use logging::{ColorCode, LogIf};
use memory::{KernelMappingIf, PageTable, PhysAddr, VirtAddr};
use net::HasSignalIf;
use vfs::{procfs::KernelProcIf, sys_root_dentry};
use vfs_core::{Dentry, SysRootDentryIf};

use crate::{
    mm::kernel_page_table_mut,
    processor::hart::{current_task_ref, local_hart},
};

/// Print msg with color
pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    driver::_print(format_args!("\u{1B}[{}m{}\u{1B}[m", color_code as u8, args))
}

struct LogIfImpl;

#[crate_interface::impl_interface]
impl LogIf for LogIfImpl {
    fn print_log(record: &log::Record) {
        let level = record.level();
        let level_color = match level {
            Level::Error => ColorCode::BrightRed,
            Level::Warn => ColorCode::BrightYellow,
            Level::Info => ColorCode::BrightGreen,
            Level::Debug => ColorCode::BrightCyan,
            Level::Trace => ColorCode::BrightBlack,
        };
        let args_color = match level {
            Level::Error => ColorCode::BrightRed,
            Level::Warn => ColorCode::BrightYellow,
            Level::Info => ColorCode::BrightGreen,
            Level::Debug => ColorCode::BrightCyan,
            Level::Trace => ColorCode::BrightBlack,
        };
        let line = record.line().unwrap_or(0);
        let target = record.file().unwrap_or("");
        let args = record.args();
        let hid = local_hart().hart_id();
        let pid = if local_hart().has_task() {
            current_task_ref().pid().to_string()
        } else {
            "-".to_string()
        };
        let tid = if local_hart().has_task() {
            current_task_ref().tid().to_string()
        } else {
            "-".to_string()
        };

        // Directly create a single format_args call with all parts embedded
        driver::_print(format_args!(
            "\u{1B}[{}m[{:>5}]\u{1B}[m\u{1B}[{}m[{:>35}:{:<4}]\u{1B}[m\u{1B}[{}m[H{},P{},T{}]\u{1B}[m \u{1B}[{}m{}\u{1B}[m\r\n",
            level_color as u8,
            level,
            ColorCode::BrightBlack as u8,
            target,
            line,
            ColorCode::BrightBlue as u8,
            hid,
            pid,
            tid,
            args_color as u8,
            args
        ));
    }
}

struct KernelPageTableIfImpl;

#[crate_interface::impl_interface]
impl KernelPageTableIf for KernelPageTableIfImpl {
    fn kernel_page_table_mut() -> &'static mut PageTable {
        kernel_page_table_mut()
    }
}

struct HasSignalIfImpl;
#[crate_interface::impl_interface]
impl HasSignalIf for HasSignalIfImpl {
    fn has_signal() -> bool {
        let task = current_task_ref();
        let mask = *task.sig_mask_ref();
        task.with_sig_pending(|pending| pending.has_expect_signals(!mask))
    }
}

struct KernelMappingIfImpl;

#[crate_interface::impl_interface]
impl KernelMappingIf for KernelMappingIfImpl {
    fn paddr_to_vaddr(paddr: PhysAddr) -> VirtAddr {
        (paddr.bits() + VIRT_RAM_OFFSET).into()
    }

    fn vaddr_to_paddr(vaddr: VirtAddr) -> PhysAddr {
        if vaddr.bits() >= VIRT_RAM_OFFSET {
            (vaddr.bits() - VIRT_RAM_OFFSET).into()
        } else {
            current_task_ref().with_mut_memory_space(|m| m.page_table().vaddr_to_paddr(vaddr))
        }
    }
}

struct KernelProcIfImpl;

#[crate_interface::impl_interface]
impl KernelProcIf for KernelProcIfImpl {
    fn exe() -> alloc::string::String {
        current_task_ref().elf().dentry().path()
    }
}

struct SysRootDentryIfImpl;

#[crate_interface::impl_interface]
impl SysRootDentryIf for SysRootDentryIfImpl {
    fn sys_root_dentry() -> Arc<dyn Dentry> {
        sys_root_dentry()
    }
}
