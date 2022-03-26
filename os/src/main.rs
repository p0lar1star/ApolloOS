// os//src/main.rs
#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(type_ascription)]

use core::arch::global_asm;

#[macro_use]
mod console;
mod batch;
mod lang_items;
mod sbi; // kernel communicate with Rust SBI
mod sync;
mod syscall;
mod trap;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, World!");
    trap::init();
    batch::init();
    println!("{}", "\nHello, batch system!");
    println!("{}", "Let's run applications\n");
    batch::run_next_app();
    // panic!("Shutdown machine!");
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
