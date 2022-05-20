// os/src/task/switch.rs

use core::arch::global_asm;
use crate::task::context::TaskContext;
global_asm!(include_str!("switch.S"));

extern "C" {
    /// 任务切换，切换任务上下文
    pub fn __switch(
        current_task_cx_ptr: *mut TaskContext,
        next_task_cx_ptr: *const TaskContext
    );
}