// os/src/mm/memory_set.rs

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;

use bitflags::*;
use lazy_static::*;
use riscv::register::satp;

use crate::config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT, USER_STACK_SIZE};
use crate::sync::UPSafeCell;

use super::{frame_alloc, FrameTracker};
use super::{PageTable, PageTableEntry, PTEFlags};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

// 创建一个内核地址空间的实例，是静态变量
// Arc<T>提供共享引用，UPSafeCell<T>提供内部可变引用访问
lazy_static! {
    /// 内核地址空间
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> = Arc::new(unsafe{
        UPSafeCell::new(MemorySet::new_kernel())
    });
}

/// 地址空间
/// 把一些由逻辑段组成的虚拟空间areas与一个运行的程序的页表page_table绑定。
/// 简而言之：地址空间 = 所有**页表项** + 多个**逻辑段**
pub struct MemorySet {
    /// 该地址空间的多级页表，
    /// PageTable下挂着所有多级页表的节点所在的物理页帧
    page_table: PageTable,
    /// 逻辑段MapArea的向量
    /// 每个MapArea下都挂着对应逻辑段中的数据所在的物理页帧
    areas: Vec<MapArea>,
}

// 地址空间的方法
impl MemorySet {
    /// 新建一个空的地址空间
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    /// 在当前地址空间插入一个Framed方式映射到物理内存的逻辑段，
    /// 调用者要保证同一地址空间内的任意两个逻辑段不能存在交集
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }
    /// 在当前地址空间插入一个新的逻辑段map_area，
    /// 如果是以相对随机方式映射到内存，可选地在那些被映射到的物理页帧上写入一些初始化数据data
    /// 先将逻辑段对应的虚拟页号
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    /// 直接在多级页表中插入一个
    /// 从 地址空间的最高虚拟页面（号） 映射到 跳板汇编代码所在的物理页面（号） 的键值对
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// Without kernel stacks.
    /// 创建内核的地址空间 MemorySet
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        // 跳板放在最高的一个虚拟页面中！
        memory_set.map_trampoline();
        // map kernel sections
        // 打印内核中各个段的起始地址和终结地址
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        // 从低地址到高地址依次创建5个逻辑段
        // 并通过push方法将它们插入到内核地址空间
        // 对于内核的.text .rodata .data .bss这四个逻辑段，全部采用恒等映射
        println!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X, // .text段可读可执行
            ),
            None,
        );
        println!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        println!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        println!("mapping physical memory");
        // 内核的地址空间中需要存在这样一个恒等映射
        // 从内核数据段结束的地方开始一直到整个内存的终结地址
        // 这样保证了启用页表机制之后，内核仍能以纯软件的方式来读写这些物理页帧
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// 以ELF格式解析出应用的各个数据段并对应生成应用的地址空间，
    /// 返回应用地址空间，用户栈栈底地址，和入口点。
    /// 栈底在高地址！在本函数中用user_stack_top标识
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        // 将跳板插入到应用地址空间的最高页面！
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        // 使用了外部crate：xmas_elf来解析传入的应用ELF数据并取出各个部分
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        // 魔数判断
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        // 得到程序头的数目
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        // 遍历所有程序头并将标记为LOAD的段加入到地址空间中
        for i in 0..ph_count {
            // 得到程序头ph
            let ph = elf.program_header(i).unwrap();
            // 检查程序头的类型
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load { // 确认程序头的类型是LOAD，这表明有被内核加载的需要
                // 找到程序头所标识的要加载到的虚拟地址区间
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;// 仅在CPU处于U特权级下才能访问
                // 根据程序头标识的权限来确定虚拟页面的权限
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                // 新建虚拟地址空间中的一个逻辑段，对应ELF文件中要映射到虚拟内存的段
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                // 从ELF文件映射到上述逻辑段
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // 开始处理用户栈
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();// max_end_vpn记录目前涉及到的最大的虚拟页号，即bss段终结的虚拟页号
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page用于保护，不进行映射，当访问到的时候就会报页错误，起到保护作用
        // 栈底地址为bss段
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // map TrapContext
        // 次高页面存放trap上下文
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        // 返回
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            // 注意切换 satp CSR 是否是一个 平滑 的过渡
            satp::write(satp);
            // 从这一刻开始 SV39 分页模式就被启用了
            // MMU 会使用内核地址空间的多级页表进行地址转换
            // 这条写入 satp 的指令及其下一条指令都在内核地址空间的代码段中
            // 在切换之前是视为物理地址直接取指，在切换之后也是一个恒等映射
            // 即使切换了地址空间，指令仍应该能够被连续的执行。
            asm!("sfence.vma");// 立即使用 sfence.vma 指令将快表清空
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
}

/// **逻辑段MapArea**描述一段地址连续的虚拟内存，
/// 即地址区间中的一段实际可用的地址连续的虚拟地址区间
pub struct MapArea {
    /// 描述一段连续的虚拟页号
    vpn_range: VPNRange,
    /// 保存该逻辑段内的虚拟页号到物理页号的映射，
    /// 拥有物理页号对应的物理页帧的所有权！RAII
    /// 仅当相对随机映射时才有用
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    /// 描述逻辑段内的所有虚拟页面映射到物理页帧的方式：是恒等映射还是相对随机映射？
    map_type: MapType,
    /// 该逻辑段的访问方式，它是页表项标志位 PTEFlags 的一个子集，仅保留 U/R/W/X 四个标志位
    /// 是否可读可写可执行？在CPU处于U特权级下能否被访问？
    map_perm: MapPermission,
}

impl MapArea {
    /// 新建一个新的逻辑段，即MapArea结构体
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        // 对起始虚拟地址向下取整
        let start_vpn: VirtPageNum = start_va.floor();
        // 对终止虚拟地址向上取整
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    // map和unmap的实现取决于映射方式：是恒等映射还是相对随机映射？
    /// 在多级页表中进行键值对的插入，
    /// 也就是填充一个页表项，需要要提供虚拟页号和页表
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            // 如果是恒等映射，那么虚拟页号=物理页号
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            // 如果是相对随机映射，需要分配一个物理页帧
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        // 页表项标志位取决于逻辑段的映射方式，即self.map_perm
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        // 插入页表项
        // 冲突问题？如果在一个地址空间内，同时启用恒等映射和相对随机映射，可能会引发冲突导致map函数中panic！
        // 当然，不会出现这种情况
        page_table.map(vpn, ppn, pte_flags);
    }
    #[allow(unused)]
    /// 删除一个页表项
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            // 回收相对随机映射得到的物理页帧
            self.data_frames.remove(&vpn);
        }
        // 恒等映射得到的物理页帧在哪里回收？
        // 与相对随机映射相比，恒等映射不需要新分配一个物理页帧
        // 恒等映射方式主要是用于：启用多级页表之后，内核仍能够在虚存地址空间中访问一个特定的物理地址指向的物理内存。
        // 所以无需回收
        // That is a question.
        page_table.unmap(vpn);
    }
    /// 将当前逻辑段到物理内存的映射，
    /// 加入到**当前逻辑段所属的地址空间**的多级页表中
    /// 也就是填充页表项
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    #[allow(unused)]
    /// 删除当前逻辑段到物理内存的映射
    /// 也就是清除页表项
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    /// 将data中的数据拷贝到当前逻辑段对应的各个物理页帧上
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        // 为什么？
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        // 切片data中的数据长度不能超过当前逻辑段的总大小
        let len = data.len();
        // 循环遍历每一个需要拷贝数据的虚拟页面
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)// 由页表和虚拟页号找到页表项
                .unwrap()
                .ppn()// 由页表项找到物理页号
                .get_bytes_array()[..src.len()];// 物理页号得到对应的切片
            // 此处是否还应该判断 该页是否可写？再考虑复制
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// 描述逻辑段内的所有虚拟页面映射到物理页帧的同一种方式
pub enum MapType {
    /// 恒等映射
    Identical,
    /// 虚地址和物理地址的映射关系相对随机
    Framed,
}

bitflags! {
    /// 表示控制该逻辑段的访问方式，它是页表项标志位 PTEFlags 的一个子集，仅保留 U/R/W/X 四个标志位
    /// 由u8类型转化而来
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

/// 检查内核地址空间的多级页表是否被正确设置
#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    // 检测代码段是否可写
    assert_eq!(
        kernel_space.page_table.translate(mid_text.floor()).unwrap().writable(),
        false
    );
    // 检测.rodata段是否可写
    assert_eq!(
        kernel_space.page_table.translate(mid_rodata.floor()).unwrap().writable(),
        false,
    );
    // 检测.data段是否可执行
    assert_eq!(
        kernel_space.page_table.translate(mid_data.floor()).unwrap().executable(),
        false,
    );
    println!("remap_test passed!");
}