// os/src/timer.rs
use riscv::register::time;
use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;

// TICKS_PER_SEC表示每秒产生的中断次数
const TICKS_PER_SEC: usize = 100;
// 一秒等于十的六次方微秒
const MICRO_PER_SEC: usize = 1_000_000;

// 取得当前mtime计数器的值
pub fn get_time() -> usize {
    time::read()
}

// 设置mtimecmp的值，相当于设置下一次中断的时刻
// CLOCK_FREQ是时钟频率，单位为赫兹，即一秒钟内mtime计数器的增量
// CLOCK_FERQ / TICKS_PER_SEC是下一次时钟中断时计数器的增量值
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

// 以微秒为单位返回当前计数器mtime的值
// CLOCK_FREQ / MICRO_PER_SEC为每微秒内计数器mtime的增量
pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / MICRO_PER_SEC)
}