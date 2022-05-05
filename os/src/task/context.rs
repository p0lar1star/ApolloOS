use crate::trap::trap_return;

#[repr(C)]
/// 任务上下文，被保存在任务控制块TaskControlBlock中，
/// 保存ra：记录任务切换回来后ret到正确的位置
/// 保存sp，记录栈顶，
/// 保存s0~s11寄存器，
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
    /// 构造应用的trap上下文用于应用的初始化
    /// 此时还在内核态，内核将这个任务上下文放到应用的内核栈中
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
