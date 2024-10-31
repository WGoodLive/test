mod context;
mod switch;
#[allow(clippy::module_inception)] // 允许跳过重复警告
mod task;
mod pid;
pub use processor::run_tasks;
use signal::{SignalFlags, MAX_SIG};
use crate::{config::*, fs::{inode::open_file, OpenFlags}};
use alloc::sync::Arc;
use lazy_static::*;
use manager::{add_task, remove_from_pid2task};
use processor::{schedule, take_current_task};
pub mod signal;
pub mod action;

use context::*;
use task::*;
pub mod manager;
pub mod processor;
pub use processor::*;

lazy_static!{
    pub static ref INITPROC:Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

pub fn add_initproc(){
    add_task(INITPROC.clone()); // 这里为什么要克隆？？
}


pub fn suspend_current_and_run_next(){

    let task = take_current_task().unwrap();
    
    let mut task_inner = task.inner_exclusive_access();

    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    add_task(task); // task没有drop，因为他所有权在这个函数转移了
    schedule(task_cx_ptr);
}

pub fn exit_current_and_run_next(exit_code:i32) {
    let task = take_current_task().unwrap(); // 这个会拿出当前任务所有权
    
    // remove from pid2task
    remove_from_pid2task(task.getpid());

    let mut inner = task.inner_exclusive_access();

    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;
    
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter(){
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }

    // 这个进程退出了，所以孩子进程也要被干掉
    inner.children.clear(); // 这里没实现RAII,因为不能因为父亲不在了，直接把孩子进程删了
    inner.memory_set.recycle_data_pages(); 
    drop(inner);
    drop(task);

    // 任务切换，当前任务的上下文被保存在相应的内核栈中，此时内核栈还没回收
    // 但是由于这个应用不会再加入运行队列执行，所以上下文可以置0
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

pub fn current_add_signal(signal:SignalFlags){
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.signals |= signal;
}

/// 信号处理函数，会修改frozen,killed等数值
pub fn handle_signals(){
    // 进程如果被暂停，这个loop会一直执行
    loop{
        // 处理信号
        check_pending_signals();
        let (frozen,killed) = {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            (task_inner.frozen, task_inner.killed)
        };
        if !frozen || killed { // 进程没有被暂停，或，进程被杀死，就跳出
            break;
        }
        suspend_current_and_run_next();
    }
}

fn check_pending_signals(){
    for sig in 0..(MAX_SIG + 1){// [0,MAX_SIG=31]
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        // 获取数字对应的信号编号
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        // 任务控制块待处理信号有这个信号，而且信号不在掩码里，就处理
        if task_inner.signals.contains(signal) && (!task_inner.signal_mask.contains(signal)) {
            let mut masked = true;
            let handling_sig = task_inner.handling_sig;
            
            if handling_sig == -1 {
                // 如果目前没有正在处理的信号
                masked = false;
            } else {
                // 当前有正在处理的信号，判断这个信号的函数处理例程是否包含这个刚来的信号
                let handling_sig = handling_sig as usize;
                if !task_inner.signal_actions.table[handling_sig]
                    .mask
                    .contains(signal)
                {
                    // 函数掩码中不存在这个函数的掩码
                    masked = false;
                }
            }

            if !masked {
                // 新的信号可以执行
                drop(task_inner);
                drop(task);
                if signal == SignalFlags::SIGKILL // 自杀信号
                    || signal == SignalFlags::SIGSTOP // 暂停信号
                    || signal == SignalFlags::SIGCONT // 继续执行信号
                    || signal == SignalFlags::SIGDEF // 默认信号处理
                {
                    // 信号是一个内核信号
                    call_kernel_signal_handler(signal);
                } else {
                    // 信号是一个用户信号，使用用户定义处理例程
                    call_user_signal_handler(sig, signal);
                    return;
                }
            }
        }
    }
}

fn call_kernel_signal_handler(signal:SignalFlags){
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            task_inner.frozen = true; 
            task_inner.signals ^= SignalFlags::SIGSTOP; // 异或，也就是消除Stop这个型号，已经处理了
        }
        SignalFlags::SIGCONT => {
            if task_inner.signals.contains(SignalFlags::SIGCONT) {
                task_inner.signals ^= SignalFlags::SIGCONT;
                task_inner.frozen = false;
            }
        }
        _ => {
            // kill或者默认都是解决进程,这个状态修改大可不必，但是为了方便handle_signals()的跳出...
            task_inner.killed = true;
        }
    }
}

fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();

    let handler = task_inner.signal_actions.table[sig].handler;
    if handler != 0 {
        // handle不等于0，那么使用默认处理

        // 这个信号处理过了，消除他
        task_inner.handling_sig = sig as isize;
        task_inner.signals ^= signal;

        // 把trap_cx储存起来
        let mut trap_ctx = task_inner.get_trap_cx();
        task_inner.trap_ctx_backup = Some(*trap_ctx);

        // 修改trap返回地址为用户空间相应信号的处理例程
        trap_ctx.sepc = handler;
        // 注意我们并没有修改 Trap 上下文中的 sp ，这意味着例程还会在原先的用户栈上执行。
        // 这是为了实现方便，在 Linux 的实现中，内核会为每次例程的执行重新分配一个用户栈


        // 函数参数就是信号量
        trap_ctx.x[10] = sig;
    } else {
        // 默认函数处理例程
        println!("[K] task/call_user_signal_handler: default action: ignore it or kill process");
    }
}

pub fn check_signals_error_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.signals.check_error()
}