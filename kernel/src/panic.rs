//! The panic handler
use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};

use arch::interrupts::disable_interrupt;
use backtrace::backtrace;
use logging::{disable_logging, is_log_initialized};
use sbi_print::sbi_println;
#[allow(deprecated)]
use sbi_rt::legacy::shutdown;

use crate::processor::hart::local_hart;

static PANIC_CNT: AtomicUsize = AtomicUsize::new(0);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe { disable_interrupt() };

    sbi_println!("early panic now!!!");
    if PANIC_CNT.fetch_add(1, Ordering::Relaxed) > 0 {
        disable_logging();
        if let Some(location) = info.location() {
            sbi_println!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                // Use the PanicMessage directly in the formatting
                info.message()
            );
        } else {
            // Use the PanicMessage directly in the formatting
            sbi_println!("Panicked: {}", info.message());
        }
        backtrace();
        #[allow(deprecated)]
        shutdown()
    }

    println!("panic now!!!");

    // NOTE: message below is mostly printed in log, if these messages can not be
    // printed, it means some of the message will cause panic again, check
    // `LogIf::print_log`.
    let logging_initialized = is_log_initialized();
    if let Some(location) = info.location() {
        if logging_initialized {
            log::error!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                info.message()
            );
        } else {
            println!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                info.message()
            );
        }
    } else {
        let msg = info.message();
        if logging_initialized {
            log::error!("Panicked: {}", msg);
        } else {
            println!("Panicked: {}", msg);
        }
    }

    log::error!("=============== BEGIN BACKTRACE ================");
    backtrace();
    log::error!("=============== END BACKTRACE ================");

    #[allow(deprecated)]
    shutdown()
}
