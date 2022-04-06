mod switch;
mod context;
mod task;

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx};
use crate::sync::UPSafeCell;
use lazy_static::*;
use switch::__switch;
use task::{TaskControlBlock, TaskStatus};
pub use context::TaskContext;

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: [TaskControlBlock; MAX_APP_NUM],
    current_task: usize,
}

// os/src/task/mod.rs
// 重用并扩展之前初始化TaskManager的全局实例TASK_MANAGER
lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        // 调用loader子模块提供的get_num_app接口获取链接到内核的应用总数
        let num_app = get_num_app();
        // 创建一个初始化的tasks数组，其中的每个任务控制块的运行状态都是Uninit
        let mut tasks = [
            TaskControlBlock {
                task_cx: TaskContext::zero_init(), // 初始化
                task_status: TaskStatus::UnInit // 未初始化状态
            };
            MAX_APP_NUM
        ];
        // 依次对每个任务控制块进行初始化，将其运行状态设置为Ready，表示可以运行
        // 并依次初始化它的任务上下文
        for i in 0..num_app {
            tasks[i].task_cx = TaskContext::goto_restore(init_app_cx(i));
            tasks[i].task_status = TaskStatus::Ready; // 准备运行状态
        }
        // 创建TaskManager实例并返回
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

pub fn suspend_current_and_run_next() {
    mark_current_suspend();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

fn mark_current_suspend() {
    TASK_MANAGER.mark_current_suspend();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

impl TaskManager {
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        unsafe {
            __switch(
                &mut _unused as *mut TaskContext,
                next_task_cx_ptr,
            );
        }
        panic!("Unreachable in run_first_task!");
    }

    fn mark_current_suspend(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn run_next_task(&self) {
        // find_next_task方法尝试寻找一个运行状态为Ready的应用并返回其id
        // 返回的类型是Option<usize>，因为不一定能找到，找不到返回的是None
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &mut inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            // 必须要手动drop掉，否则不能读写Task_Manager.inner
            unsafe {
                __switch(
                    current_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
        } else {
            panic!("All applications completed!");
        }
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        // tasks是一个固定的任务控制块组成的表，长度为num_app
        // 可以用下标0~num_app-1来访问得到每个应用的控制状态
        // 这里是为了找到current_task后面的第一个状态为Ready的应用
        // 从current_task+1开始循环一圈
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| {
                inner.tasks[*id].task_status == TaskStatus::Ready
            })
    }
}