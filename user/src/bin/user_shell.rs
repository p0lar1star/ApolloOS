#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

extern crate alloc;

#[macro_use]
extern crate user_lib;

/// 换行：把光标垂直移动到下一行，0x0a
const LF: u8 = 0x0au8;
/// 回车：把光标移动到当前行的开头,0x0d
const CR: u8 = 0x0du8;
/// 删除
const DL: u8 = 0x7fu8;
/// 退格
const BS: u8 = 0x08u8;

use alloc::string::String;
use user_lib::console::getchar;
use user_lib::{exec, fork, waitpid};

#[no_mangle]
pub fn main() -> i32 {
    println!("Rust user shell");
    // 用户输入的命令
    let mut line: String = String::new();
    print!("p0lar1s@os:~# ");
    loop {
        let c = getchar();
        match c {
            // 输入回车键，fork出一个子进程
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    line.push('\0');
                    let pid = fork();
                    // pid = 0，说明是子进程
                    if pid == 0 {
                        // child process
                        if exec(line.as_str()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!();
                    } else {
                        // 父进程
                        let mut exit_code: i32 = 0;// 用于保存子进程的退出码
                        // 父进程等待子进程退出
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!("p0lar1s@os:~# ");
            }
            // 输入退格键
            BS | DL => {
                if !line.is_empty() {
                    // 将屏幕上当前行的最后一个字符用空格替换掉
                    // 先退一格
                    print!("{}", BS as char);
                    // 再输出空格（覆盖）
                    print!(" ");
                    // 再退一格
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            // 输入其他字符，正常显示
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
