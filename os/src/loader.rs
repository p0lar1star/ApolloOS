// os/src/loader.rs

use core::arch::asm;
use crate::trap::TrapContext;
use crate::config::*;

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    // 将Trap上下文压入到内核栈
    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

// 返回第i个程序要加载到的物理内存地址
fn get_base_i(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

// 读取内核数据段上app的数量
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe {
        (_num_app as usize as *const usize).read_volatile()
    }
}

// 一次性把内核数据段上的所有应用加载到物理内存中
pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    // app_start指向内核数据段上所有程序指令的开头
    let app_start = unsafe {
        core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1)
    };
    // 清除指令缓存，因为可能需要多次加载所有app到相同的位置并执行
    unsafe {
        asm!("fence.i");
    }
    // 开始一个一个加载
    for i in 0..num_app {
        // 得到第i个程序要加载到的物理内存地址
        let base_i = get_base_i(i);
        // 将要加载程序的地方清空
        (base_i..base_i + APP_SIZE_LIMIT).for_each(|addr| unsafe {
            (addr as *mut u8).write_volatile(0);
        });
        // 把程序从内核的数据段转移到物理内存
        // app_src是第i个程序的切片，长度为程序的大小
        let app_src = unsafe {
            core::slice::from_raw_parts(
                app_start[i] as usize as *const u8,
                app_start[i + 1] - app_start[i],
            )
        };
        // app_dst是第i个程序在物理内存中的位置的切片。长度也为程序的大小
        let app_dst = unsafe {
            core::slice::from_raw_parts_mut(
                base_i as usize as *mut u8,
                app_src.len(),
            )
        };
        app_dst.copy_from_slice(app_src);
    }
}

// 返回内核栈上Trap上下文的地址，即内核栈顶
pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_context(TrapContext::app_init_context(
        get_base_i(app_id),
        USER_STACK[app_id].get_sp()
    ))
}