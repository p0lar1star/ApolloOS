// user/src/syscall.rs
use core::arch::asm;

// RISC-V 寄存器编号从 0~31,表示为 x0~x31
// x10~x17:对应 a0~a7
// x1:对应 ra
// 在 RISC-V 调用规范中,
// 约定寄存器a0~a6 保存系统调用的参数,a0 保存系统调用的返回值。
// 寄存器 a7 用来传递 syscall ID
fn syscall(id: usize, args: [usize; 3]) -> isize {
    let mut ret: isize;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x17") id
        );
    }
    ret
}

const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len() as usize])
}

pub fn sys_exit(xstate: i32) -> isize {
    syscall(SYSCALL_EXIT, [xstate as usize, 0, 0])
}
