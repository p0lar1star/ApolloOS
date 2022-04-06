// os/src/task/context.rs
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext {
    ra: usize,
    sp: usize,
    s: [usize; 12],
}

impl TaskContext {
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
    // 返回任务上下文，TaskContext类型
    pub fn goto_restore(kstatck_ptr: usize) -> Self {
        extern "C" {
            fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kstatck_ptr as usize, // 指向内核栈上的Trap上下文
            s: [0; 12],
        }
    }
}