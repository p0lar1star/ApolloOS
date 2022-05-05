// os/src/task/task.rs
use super::TaskContext;
use crate::config::{kernel_stack_position, TRAP_CONTEXT};
use crate::mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::trap::{trap_handler, TrapContext};

pub struct TaskControlBlock {
    /// 任务状态
    pub task_status: TaskStatus,
    /// 任务上下文
    pub task_cx: TaskContext,
    /// 任务的地址空间
    pub memory_set: MemorySet,
    /// trap页面物理页号，位于应用地址空间次高页
    pub trap_cx_ppn: PhysPageNum,
    /// 应用数据的大小，从应用地址空间0x0到用户栈结束一共多少字节，暂时不考虑堆，
    /// 相当于记录了用户栈的栈底（高地址）
    pub base_size: usize,
}

impl TaskControlBlock {
    /// 查找应用的Trap上下文的内核虚地址，
    /// 返回对Trap上下文的可变引用，
    /// 即Trap上下文的物理地址
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        // PhysPageNum::get_mut 是一个泛型函数，由于我们已经声明了总体返回 TrapContext 的可变引用，
        // 则Rust编译器会给 get_mut 泛型函数针对具体类型 TrapContext 的情况生成一个特定版本的 get_mut 函数实现。
        // 在 get_trap_cx 函数中则会静态调用``get_mut`` 泛型函数的特定版本实现。
        self.trap_cx_ppn.get_mut()
    }
    /// 得到当前应用地址空间对应的的token（satp寄存器）
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// 解析传入的elf格式文件 并 构造应用的地址空间memory_set
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        // 得到应用地址空间memory_set，用户栈栈底（高地址！）user_sp，和入口点
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 查多级页表
        // 找到应用地址空间中的Trap上下文对应的物理页号
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // map a kernel-stack in kernel space
        // 根据传入的应用id找到应用的内核栈在内核地址空间中的位置
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        // 向内核地址空间中插入 该应用的内核栈 这个逻辑段，权限为可读可写
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        // 为程序新建任务控制块
        // 在应用的内核栈顶压入一个跳转到trap_return的上下文
        let task_control_block = Self {
            task_status,
            // 在应用的内核栈顶写入构造好的任务上下文
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,// 应用的地址空间中trap上下文对应的物理页号
            base_size: user_sp,
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();// 查找应用空间的trap上下文在内核地址空间中的虚地址
        // 调用app_init_context通过Trap上下文的可变引用来进行初始化
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    // 准备运行
    Running,
    // 正在运行
    Exited,// 已退出
}
