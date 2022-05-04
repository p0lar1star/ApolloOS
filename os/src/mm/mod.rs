mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, PageTableEntry};
use page_table::{PTEFlags, PageTable};

/// 内存管理系统的初始化
pub fn init() {
    // 全局动态内存分配器的初始化
    heap_allocator::init_heap();
    // 初始化物理页帧管理器
    frame_allocator::init_frame_allocator();
    // 开启分页模式
    // 当一个函数接受类型为 &mut T 的参数却被传入一个类型为 &mut RefMut<'_, T> 的参数的时候
    // 编译器会自动进行类型转换使参数匹配
    KERNEL_SPACE.exclusive_access().activate();
    // 自此，启用了内核动态内存分配，物理页帧管理和分页模式
}
