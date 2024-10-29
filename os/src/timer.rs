use riscv::register::time;
const MSEC_PRE_SEC:usize = 1000;
use crate::config::CLOCK_FREQ; 
//常数 CLOCK_FREQ 是一个预先获取到的各平台不同的时钟频率，单位为赫兹，也就是一秒钟之内计数器的增量
const TICKS_PER_SEC: usize = 100;
// RustSBI已经预留了相应的接口,
// 接口被riscv封装
/// 获取当前 mtime 计数器的值
pub fn get_time()->usize{
    time::read()
}



///  以毫秒为单位返回当前计数器的值
pub fn get_time_ms() -> usize{
    get_time() / (CLOCK_FREQ / MSEC_PRE_SEC)
}

use crate::sbi::set_timer;

/// 设置mtimecpp(m特权级寄存器)
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

// pub fn init_timer(){
//     crate::trap::enable_timer_interrupt(); // sie 使能
//     set_next_trigger();
// }
