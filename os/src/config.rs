pub use crate::board::CLOCK_FREQ;

pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const MEMORY_END: usize = 0x80800000;
pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SIZE_BITS: usize = 0xc;

/// Trampoline页面起始地址，最高的一个页面
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
/// TrapContext页面起始地址，次高的一个页面
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

/// Return (bottom, top) of a kernel stack in kernel space.
/// 返回应用的**内核栈**在内核地址空间中的位置
/// (低地址，高地址）
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}