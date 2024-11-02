use core::cmp::Ordering;
use alloc::collections::binary_heap::BinaryHeap;
use lazy_static::lazy_static;
use alloc::sync::Arc;
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
use crate::sync::UPSafeCell;
use crate::task::TaskControlBlock;

/// 设置mtimecpp(m特权级寄存器)
pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}

// pub fn init_timer(){
//     crate::trap::enable_timer_interrupt(); // sie 使能
//     set_next_trigger();
// }

/// 为了实现sleep系统调用的唤醒功能  
/// 线程等待的事件则是 时钟计数器 的值超过当前时间再加上线程睡眠的时长的总和，也就是超时之后就可以唤醒线程了。
pub struct TimerCondVar {
    pub expire_ms: usize,
    pub task: Arc<TaskControlBlock>,
}

/// 定义相等
impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}
impl Eq for TimerCondVar {}

impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let a = -(self.expire_ms as isize);
        let b = -(other.expire_ms as isize);
        Some(a.cmp(&b))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

lazy_static! {
    /// - 即在每次时钟中断的时候检查在上个时间片中是否有一些线程的睡眠超时了，如果有的话我们就唤醒它们
    /// - 当时钟中断的时候我们可以扫描所有的 TimerCondVar ，将其中已经超时的移除并唤醒相应的线程。
    /// - 可以以超时时间为键值将所有的 TimerCondVar 组织成一个小根堆（另一种叫法是优先级队列）
    static ref TIMERS: UPSafeCell<BinaryHeap<TimerCondVar>> =
        unsafe { UPSafeCell::new(BinaryHeap::<TimerCondVar>::new()) };
}

/// 增加新的sleep休眠线程
pub fn add_timer(expire_ms: usize, task: Arc<TaskControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    timers.push(TimerCondVar { expire_ms, task });
}

/// 移出某个线程的timers
pub fn remove_timer(task: Arc<TaskControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    let mut temp = BinaryHeap::<TimerCondVar>::new();
    for condvar in timers.drain() {
        if Arc::as_ptr(&task) != Arc::as_ptr(&condvar.task) {
            temp.push(condvar);
        }
    }
    timers.clear();
    timers.append(&mut temp);
}

/// 检查有那些需要被唤醒
pub fn check_timer() {
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access();
    while let Some(timer) = timers.peek() {
        if timer.expire_ms <= current_ms {
            wakeup_task(Arc::clone(&timer.task));
            timers.pop();
        } else {
            break;
        }
    }
}