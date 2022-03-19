// os//src/main.rs
#![no_std]
#![no_main]
#![feature(panic_info_message)]

mod console;
mod lang_items;
mod sbi; // kernel communicate with Rust SBI
use core::arch::global_asm;
use sbi::{console_putchar, shutdown};
global_asm!(include_str!("entry.asm"));

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    println!("Hello World!");
    // shutdown();
    panic!("Shutdown machine!");
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
