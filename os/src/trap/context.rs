use riscv::register::sstatus::{self, Sstatus, SPP};
// os/src/trap/context.rs
#[repr(C)]
pub struct TrapContext {
    pub x: [usize; 32],
    pub sstatus: Sstatus, // sstatus
    pub sepc: usize,     // return addr
}

impl TrapContext {
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    // app上下文初始化，此时还处在S特权级
    // 设置sstatus为U特权级
    // 设置sepc指向程序指令的起始地址0x80400000
    // 设置sp指针指向用户栈的栈顶
    // 返回上下文
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
        };
        cx.set_sp(sp: usize);
        cx
    }
}
