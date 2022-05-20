#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[cfg(feature = "board_k210")]
#[path = "boards/k210.rs"]
mod board;
#[cfg(not(any(feature = "board_k210")))]
#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod mm;
mod sbi;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

use core::arch::global_asm;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    println!("    _                _ _        ___  ____");
    println!("   / \\   _ __   ___ | | | ___  / _ \\/ ___|");
    println!("  / _ \\ | '_ \\ / _ \\| | |/ _ \\| | | \\___ \\");
    println!(" / ___ \\| |_) | (_) | | | (_) | |_| |___) |");
    println!("/_/   \\_\\ .__/ \\___/|_|_|\\___/ \\___/|____/");
    println!("        |_|");
    clear_bss();
    println!("[kernel] Hello, World!");
    println!("[kernel] Now init the memory manager...");
    mm::init();
    println!("[kernel] back to rust_main!");
    mm::remap_test();
    // 第一个加载init_proc
    task::add_initproc();
    println!("after initproc!");
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    loader::list_apps();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}