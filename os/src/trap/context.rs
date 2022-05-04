use riscv::register::sstatus::{self, SPP, Sstatus};

// os/src/trap/context.rs
#[repr(C)]
/// Trap上下文
/// 它们在应用初始化的时候由内核写入应用地址空间中的TrapContext的相应位置
pub struct TrapContext {
    /// 32个通用寄存器
    pub x: [usize; 32],
    /// 保存当前的特权级
    pub sstatus: Sstatus,
    /// 返回地址
    pub sepc: usize,
    /// satp存有内核地址空间页表的物理页号，在应用初始化的时候由内核写入
    pub kernel_satp: usize,
    /// 应用的内核栈栈底（高地址）的虚拟地址，在应用初始化时由内核写入
    pub kernel_sp: usize,
    /// 内核中trap handler入口点的虚拟地址，在应用初始化时由内核写入
    pub trap_handler: usize,
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    /// app上下文初始化，此时还处在S特权级
    /// 设置sstatus为U特权级,
    /// 设置sepc指向程序指令的起始地址,
    /// 设置sp指针指向用户栈的栈顶,
    /// 设置kernel_satp保存内核地址空间对应的satp寄存器
    /// 设置kernel_sp指向用户栈的栈底（高地址）
    /// 设置trap_handler指向内核中trap handler入口点的虚拟地址
    /// 返回上下文
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        cx.set_sp(sp);
        cx
    }
}
