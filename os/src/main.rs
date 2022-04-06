// os//src/main.rs
#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::arch::global_asm;

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;
#[macro_use]
mod console;
mod lang_items;
mod sbi;
mod sync;
mod syscall;
mod loader;
mod config;
mod trap;
mod task;
mod timer;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, World!");
    // 先初始化中断向量
    trap::init();
    // 从内核的数据段加载所有应用程序到物理内存
    loader::load_apps();
    // 避免S特权级时钟中断被屏蔽
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    for a in sbss as usize..ebss as usize {
        unsafe {
            (a as *mut usize).write_volatile(0);
        }
    }
}
