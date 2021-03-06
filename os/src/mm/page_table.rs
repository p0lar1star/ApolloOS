// os/src/mm/page_table.rs
use super::{frame_alloc, FrameTracker, PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

bitflags! {
    /// 页表中的标志位PTEFlags
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

/// 页表项数据结构：64位
/// 10~53这44bits对应物理页号
/// 8~9这两bits为RSW，留给S特权级软件（也就是内核）自行决定如何使用
/// 0~7这8bits对应各项标志位
#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    /// 由 PhysPageNum 和 PTEbits 生成页表项
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> PageTableEntry {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    /// 生成一个所有位为0的空页表项，由于V=0，所以是不合法的
    pub fn empty() -> Self {
        PageTableEntry {
            bits: 0,
        }
    }
    /// 传入页表项，由页表项得到物理页号
    pub fn ppn(&self) -> PhysPageNum {
        ((self.bits >> 10) & ((1usize << 44) - 1)).into()
    }
    /// 由页表项得到标志位
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    /// 快速判断页表项的V标志位是否为1:判断两个集合的交集是否为空集
    pub fn is_valid(&self) -> bool {
        // 8bitsflags & 00000001 == 00000000?false:true
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// 判断是否可读
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// 判断是否可写
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// 判断是否可执行
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// PageTable类型用于描述某个应用的地址空间对应的页表，我将其称之为总页表，
/// PageTable不仅保存**页表根节点**的物理页号（root_ppn），还保存
/// **页表所有节点**（包括根节点）所在的物理页号。（FrameTracker是物理页号的封装）
pub struct PageTable {
    root_ppn: PhysPageNum,
    /// 向量frames以FrameTracker的形式保存了页表所有节点所在的物理页帧
    /// 它把FrameTracker的生命周期进一步绑定到PageTable下面
    /// 当PageTable生命周期结束后，向量frames里面的那些FrameTracker也被回收了
    /// 也就意味着**存放页表节点**的那些物理页帧被回收了
    frames: Vec<FrameTracker>,// RAII!
}

impl PageTable {
    /// 创建一个新的页表
    pub fn new() -> Self {
        // 创建时只需有一个根节点，保存根节点的物理页号root_ppn
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    /// 临时创建一个专用来手动查页表的PageTable，传入satp寄存器的值
    /// satp寄存器中前44位存的是根页表所在的物理页号
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    /// 根据虚拟页号，在多级页表的各个节点中找到一个虚拟页号对应的页表项
    /// 找不到就创建，返回对这个页表项的可变引用
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            // 每次取出当前节点对应的物理页帧上的长度为512的页表项数组
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i == 2 {
                result = Some(pte);
                break;
            }
            // 如果这个页表项节点不存在，也就是无效，那么新创建一个页表项节点
            // 也就是分配一个页面给这个页表项节点，并将页面的标志位置为有效
            // 但是不修改叶节点，因为i=2时已返回
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // 还要将新分配的物理页帧移动到向量frames中方便后续的自动回收
                self.frames.push(frame);
            }
            // 更新物理页号：将页表项转化成物理页号
            ppn = pte.ppn();
        }
        result
    }

    /// 根据虚拟页号，在多级页表中找一个与其对应的页表项,
    /// 找不到则返回None，找到则返回页表项的可变引用
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    #[allow(unused)]
    /// 为了动态维护一个虚拟页号到页表项的映射，支持插入和删除键值对，
    /// 通过map方法在多级页表中插入一个键值对
    /// 要求传入虚拟页号、物理页号和标志位
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        // 如果页表项是有效的，直接panic，因为这表示该页已被分配
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    #[allow(unused)]
    /// 通过unmap方法来删除一个键值对，仅需给出作为索引的虚拟页号
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    /// 手动查找页表项：如果能够找到页表项，那么将页表项拷贝一份并返回
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| { pte.clone() })
    }
    /// 根据传入的虚拟地址返回对应的Option<物理地址>
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            //println!("translate_va:va = {:?}", va);
            let aligned_pa: PhysAddr = pte.ppn().into();
            //println!("translate_va:pa_align = {:?}", aligned_pa);
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            // 返回虚拟地址对应的物理地址
            (aligned_pa_usize + offset).into()
        })
    }
    /// 按照satp CSR格式要求构造一个无符号64位整数
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// 传入当前应用的token，buf的虚拟地址和长度，返回对u8缓冲区的可变引用
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);    // 构建临时页表
    let mut start = ptr as usize; // 缓冲区buf起始地址
    let end = start + len; // 缓冲区buf终止地址
    // v用于保存对u8缓冲区的可变引用，由于缓冲区可能因为不在同一页要被分成多段，所以使用vec保存多个引用
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        // 将缓冲区起始地址strat_va转换成虚拟页号继而转化成物理页号
        let ppn = page_table.translate(vpn).unwrap().ppn();
        // 得到下一页面的页面号
        vpn.step();
        // 下一页面的页面号转化成当前页面的终止地址
        let mut end_va: VirtAddr = vpn.into();
        // 缓冲区终止地址 > 当前页面的终止地址？
        // 如果是，那么先取 缓冲区终止地址 和 当前页面终止地址的 最小值，也就是当前页面的终止地址
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            // 对于buf超过当前这一页的情况
            // push 一整物理页上 从start_va的偏移地址开始到一整页终止 u8字节数组 的可变引用
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            // 对于buf未超过当前这一页的情况
            // push 一整物理页上 从start_va的偏移地址开始到end_va的偏移地址结束 u8字节数组 的可变引用
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        // 对于buf超过当前这一页
        start = end_va.into();
    }
    v
}

/// 在内核中查找用户地址空间中的字符串。
// 这个函数签名可能设计的不太好，因为ptr是应用名字字符串在用户地址空间的起始地址，用usize类型比较好
pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    // 这里可能也设计的不太好？对于物理地址仅进行一次查询即可，没必要每次循环都根据虚拟地址来查物理地址，每次循环物理地址+1即可
    // 非也，万一不在同一个页面呢？还是有必要对于每个虚拟地址都查找物理地址的
    loop {
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .get_mut());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    //println!("into translated_refmut!");
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    //println!("translated_refmut: before translate_va");
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_mut()
}