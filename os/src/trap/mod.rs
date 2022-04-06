// os/src/syscall/mod.rs
use core::arch::global_asm;

use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    stval, stvec, sie,
};

pub use context::TrapContext;

use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::set_next_trigger;
use crate::syscall::syscall;

// os/src/trap/mod.rs
mod context;

global_asm!(include_str!("trap.S"));

pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        // 设置中断向量表的地址为__alltraps
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    // 根据 scause 寄存器所保存的 Trap 的原因进行分发处理
    // 这里我们无需手动操作这些 CSR 
    // 而是使用 Rust 的 riscv 库来更加方便的做这些事情。
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            println!("[kernel] PageFault in application, kernel killed it.");
            exit_current_and_run_next();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!("Unsupported trap {:?}, stval = {:#x}!", scause.cause(), stval);
        }
    }
    cx
}

// 设置sie.stie为1使S特权级时钟中断不会被屏蔽
pub fn enable_timer_interrupt() {
    unsafe{
        sie::set_stimer();
    }
}