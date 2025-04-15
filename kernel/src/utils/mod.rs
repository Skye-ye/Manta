use alloc::{format, sync::Arc};

use arch::timer::timelimited_task::{ksleep_ms, ksleep_s};
use config::process::INIT_PROC_PID;

use crate::task::{self, TASK_MANAGER, Task};

/// Used for debug.
#[allow(unused)]
pub fn exam_hash(buf: &[u8]) -> usize {
    let mut h: usize = 5381;
    for c in buf {
        h = h.wrapping_mul(33).wrapping_add(*c as usize);
    }
    h
}

/// Spawn a timer task that runs a function periodically.
#[allow(unused)]
pub fn spawn_timer_tasks<F>(f: F, interval_secs: usize)
where
    F: FnOnce() + Send + Copy + 'static,
{
    task::spawn_kernel_task(async move {
        let f = f;
        loop {
            f();
            ksleep_s(interval_secs).await;
        }
    });
}

#[allow(unused)]
pub fn spawn_timer_tasks_ms<F>(f: F, interval_millisecs: usize)
where
    F: FnOnce() + Send + Copy + 'static,
{
    task::spawn_kernel_task(async move {
        let f = f;
        loop {
            f();
            ksleep_ms(interval_millisecs).await;
        }
    });
}

#[allow(unused)]
pub fn print_proc_tree() {
    fn dfs_print(proc: Arc<Task>, level: usize, prefix: &str) {
        let indent = " ".repeat(level * 4);
        println!("{}{}{}", indent, prefix, proc.args_ref().join(" "));
        for (i, child) in proc.children().iter() {
            dfs_print(child.clone(), level + 1, &format!("P{i} -- "));
        }
    }

    if let Some(init) = TASK_MANAGER.get(INIT_PROC_PID) {
        dfs_print(init, 0, "P1 -- ");
    }
}
