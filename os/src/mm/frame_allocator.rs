use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::*;

/// 对PhysPageNum的进一步封装，基于RAII思想
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

// 为FrameTracker实现Drop Trait
impl Drop for FrameTracker {
    /// 当一个FrameTracker实例被回收的时候，它的drop方法会自动被编译器调用，
    /// 即自动回收了物理页帧，以供后续使用，
    /// 有了它，我们就不必手动回收物理页帧了，在编译器就解决了很多潜在的问题
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

/// 作为一个物理页帧管理器，需要实现这个trait
trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// 栈式物理页帧管理器的声明
/// 包含 当前可分配的物理页号区间 和 已回收的物理页号
pub struct StackFrameAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    /// 初始化物理页帧管理器，参数为可用页帧号的左右区间
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}

// 这里是具体实现
impl FrameAllocator for StackFrameAllocator {
    /// 创建物理页帧管理器实例，将区间两端设为0
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }
    /// 分配物理页帧
    fn alloc(&mut self) -> Option<PhysPageNum> {
        // 若存在已经回收的页面，直接分配已经回收的
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else {
            // 否则先检查是否还有空余页帧
            if self.current == self.end {
                // 不存在空闲页帧
                None
            } else {
                // 存在空闲页帧
                self.current += 1;
                Some((self.current - 1).into())
            }
        }
    }
    /// 回收物理页帧
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // 合法性检查
        if ppn >= self.current || self.recycled
            .iter()
            .find(|&v| { *v == ppn })
            .is_some() {
            panic!("Frame ppn {:#x} has not been allocated!", ppn);
        }
        // 回收
        self.recycled.push(ppn);
    }
}

/// StackFrameAllocator的全局实例：FRAME_ALLOCATOR，
/// FrameAllocatorImpl是StackFrameAllocator类型的别名
/// 这里使用UPSafeCell对这个全局变量进行包装
/// 每次对分配器操作时，都需要通过FRAME_ALLOCATOR.exclusive_access()拿到分配器的可变借用
// 什么是RAII？
type FrameAllocatorImpl = StackFrameAllocator;
lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> = unsafe {
        UPSafeCell::new(FrameAllocatorImpl::new())
    };
}

/// 物理页帧全局管理器FRAME_ALLOCATOR初始化
/// 根据ekernel和MEMORY_END指定可分配的物理页帧
pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access().init(PhysAddr::from(ekernel as usize).ceil(), PhysAddr::from(MEMORY_END).floor());
}

/// 给其它内核模块调用的分配物理页帧的接口，
/// 返回值并不是FrameAllocator要求的物理页号！
/// 这是一种RAII的思想，
/// 将一个物理页帧的生命周期绑定到一个FrameTracker变量上。
pub fn frame_alloc() -> Option<FrameTracker> {
    // 将每个分配来的物理页帧的页号都作为参数传给FrameTracker的new方法来创建一个FrameTracker实例
    FRAME_ALLOCATOR.exclusive_access().alloc().map(|ppn| FrameTracker::new(ppn))
}

/// 回收物理页帧的接口
fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}