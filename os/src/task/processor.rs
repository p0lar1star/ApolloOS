use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// 处理器管理结构，包含：
/// 指向当前处理器上正在运行的进程的任务控制块的指针和
/// idle控制流的任务上下文
pub struct Processor {
    /// 当前处理器上正在执行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 处理器上的**idle控制流**的任务上下文
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    /// idle任务上下文的物理地址
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    /// 取出正在运行的任务，返回指向该任务的任务控制块的Arc指针，self.current变成None
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        // take方法可以从Option中取出值，原来的Option变量变成None值
        self.current.take()
    }
    /// 返回指向当前任务控制块的Arc指针，但是self.current并不会变成None，指针计数+1
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        // as_ref该方法将Option变量或对Option的引用变为 对Option所包含对象的不可变引用，并且返回一个新的Option。
        // 可以对这个新的Option进行unwrap操作，可以获得原Option所包含的对象的不可变引用。
        // 比如这里as_ref返回的就是Option<&Arc<TaskControlBlock>>，map对其中的&Arc<TaskControlBlock>进行操作
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

// 单核CPU，仅单个Processor的全局实例
lazy_static! {
    /// 全局的处理器管理器，由于目前只支持单核，所以只有一个实例
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

/// idle控制流，
/// 内核初始化完毕之后，会通过调用 `run_tasks` 函数来进入 idle 控制流
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        // fetch_task从队头取出下一个任务
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// 获得指向当前正在运行的任务的任务控制块的Arc指针，用Option包裹
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// 得到当前应用的token值
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

/// 得到对当前应用的Trap页面的可变引用
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// 当一个应用用尽了时间片或主动yield，本函数使CPU切换到idle控制流。
/// 需要传入即将被切换出去的任务的 task_cx_ptr
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
