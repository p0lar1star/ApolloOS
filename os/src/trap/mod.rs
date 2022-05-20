mod context;

use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::syscall::syscall;
use crate::task::{
    current_trap_cx, current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
};
use crate::timer::set_next_trigger;
use core::arch::{asm, global_asm};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

pub fn init() {
    set_kernel_trap_entry();
}

/// 设置内核状态下的trap_entry
fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

/// 设置用户态的trap_entry
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

/// 设置sie.stie为1使S特权级时钟中断不会被屏蔽
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
/// trap处理函数
pub fn trap_handler() -> ! {
    // 设置内核状态下的trap_entry，在内核下不允许trap，直接panic
    set_kernel_trap_entry();
    // trap的原因是什么？
    // scause/stval在trap时由硬件分别被修改成这次 Trap 的原因以及相关的附加信息。
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // 获得当前应用的Trap上下文的可变引用
            let mut cx = current_trap_cx();
            // 进入Trap的时候，硬件会将sepc设置为这条ecal指令所在的地址，随后在alltraps中我们将sepc寄存器的值保存在Trap上下文中
            // Trap返回之后，应用程序控制流应从ecall的下一条指令开始执行，于是cx.sepc+=4
            cx.sepc += 4;
            // get system call return value
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]);
            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            // page fault exit code
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            // illegal instruction exit code
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    // 完成trap处理，返回用户态
    trap_return();
}

#[no_mangle]
/// trap处理完毕，在trap_handler最后执行本函数返回到用户态
pub fn trap_return() -> ! {
    // 设置用户态的trap_entry为__alltraps
    // 让应用 Trap 到 S 的时候可以跳转到 __alltraps
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    // 得到应用的token
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    // 计算 __restore的虚拟地址 = TRAMPOLINE + restore相对于alltraps的偏移量
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    // 使用 fence.i 指令清空指令缓存 i-cache
    // 在内核中进行的一些操作可能导致一些原先存放某个应用代码的物理页帧如今用来存放数据或者是其他应用的代码
    // i-cache 中可能还保存着该物理页帧的错误快照,导致jr跳到错误的物理页帧
    unsafe {
        asm!(
        "fence.i",
        "jr {restore_va}",
        restore_va = in(reg) restore_va,
        in("a0") trap_cx_ptr,
        in("a1") user_satp,
        options(noreturn)
        );
    }
}

#[no_mangle]
pub fn trap_from_kernel() -> ! {
    panic!("a trap {:?} from kernel!", scause::read().cause());
}

pub use context::TrapContext;