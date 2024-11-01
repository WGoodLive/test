mod context;
mod switch;
#[allow(clippy::module_inception)] // 允许跳过重复警告
mod task;
mod id;
mod process;
use id::{TaskUserRes, IDLE_PID};
use process::ProcessControlBlock;
pub use processor::run_tasks;
use signal::{SignalFlags, MAX_SIG};
use crate::{config::*, fs::{inode::open_file, OpenFlags}, sbi::shutdown};
use alloc::{sync::Arc, vec::Vec};
use lazy_static::*;
use manager::{add_task, remove_from_pid2process, remove_task};
use processor::{schedule, take_current_task};
pub mod signal;
pub mod action;

use context::*;
use task::*;
pub mod manager;
pub mod processor;
pub use processor::*;
pub use task::TaskControlBlock;
lazy_static!{
    pub static ref INITPROC:Arc<ProcessControlBlock> = {
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(v.as_slice())
    };
}

/// ????????
pub fn add_initproc() {
    let _initproc = INITPROC.clone();
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
    // 把当前任务从 正在调度的任务中 拿出来，后面让主进程的主线程进去
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let process = task.process.upgrade().unwrap();
    let tid = task_inner.res.as_ref().unwrap().tid;
    // 修改当前任务的返回码，和删除他对应的trap_cx + ustack + tid
    task_inner.exit_code = Some(exit_code);
    task_inner.res = None;
    // 现在我们不能释放线程内核栈，因为你现在正在用他
    // 他在waittid的时候别人释放
    drop(task_inner);
    drop(task);
    // 但是如果是主线程，此时进程资源也需要释放
    if tid == 0 {
        let pid = process.getpid();
        // 如果是主进程的主线程
        if pid == IDLE_PID {
            println!(
                "[kernel] Idle process exit with exit_code {} ...",
                exit_code
            );
            if exit_code != 0 {
                // 主进程退出失败
                shutdown(true);
            } else {
                // 主进程退出成功
                shutdown(false);
            }
        }
        // 其他进程的主线程
        // 先取消pid映射的控制块
        remove_from_pid2process(pid);
        let mut process_inner = process.inner_exclusive_access();
        // 进程变成僵尸进程
        process_inner.is_zombie = true;
        // 记录进程退出码 = 主线程退出码
        process_inner.exit_code = exit_code;

        {
            // 把孩子进程给主线程
            let mut initproc_inner = INITPROC.inner_exclusive_access();
            for child in process_inner.children.iter() {
                child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
                initproc_inner.children.push(child.clone());
            }
        }

        // 注意：这里其他线程的trap_cx与ustack不用释放，因为用户地址空间随着memory_set释放而释放
        // 我们现在主要释放内核的一些空间(process弱引用+线程kstack后面再弄)
        let mut recycle_res = Vec::<TaskUserRes>::new();
        for task in process_inner.tasks.iter().filter(|t| t.is_some()) {
            let task = task.as_ref().unwrap();
            // 线程/任务/调度管理器 放着 线程的强引用，需要释放
            remove_inactive_task(Arc::clone(&task));
            let mut task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.take() {
                recycle_res.push(res);
            }
        }
        // 释放tid与TaskUserRes需要process_inner，他们需要先释放，
        // 不能等线程由于没有绑定，那时系统drop回收
        drop(process_inner);
        recycle_res.clear();

        let mut process_inner = process.inner_exclusive_access();
        // 孩子都给了主进程，可以清了，否则强引用会出事，无法回收
        process_inner.children.clear();
        // 释放数据页，此时还没有释放页表，这由父亲删(他通过页表项删数据，此时怎么删自己)
        process_inner.memory_set.recycle_data_pages();
        // 删除文件
        process_inner.fd_table.clear();
        // 移出其他所有线程，内核栈也会回收了，主线程此时还在
        while process_inner.tasks.len() > 1 {
            process_inner.tasks.pop();
        }
    }
    // 主线程的内核栈还在，因为此时你就在主线程
    // 如果上面的线程不是主线程，那么他的内核栈还在，因为作为当前任务他正在用内核
    drop(process);
    // 此时转向主进程，由于是僵尸进程的任务，他不会加入调度管理器，等销毁
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

pub fn current_add_signal(signal:SignalFlags){
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.signals |= signal;
}

/// 信号处理函数，会修改frozen,killed等数值
pub fn handle_signals(){
    // 进程如果被暂停，这个loop会一直执行
    loop{
        // 处理信号
        check_pending_signals();
        let (frozen,killed) = {
            let process = current_process();
            let process_inner = process.inner_exclusive_access();
            (process_inner.frozen, process_inner.killed)
        };
        if !frozen || killed { // 进程没有被暂停，或，进程被杀死，就跳出
            break;
        }
        suspend_current_and_run_next();
    }
}

fn check_pending_signals(){
    for sig in 0..(MAX_SIG + 1){// [0,MAX_SIG=31]
        let process = current_process();
        let process_inner = process.inner_exclusive_access();
        // 获取数字对应的信号编号
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        // 任务控制块待处理信号有这个信号，而且信号不在掩码里，就处理
        if process_inner.signals.contains(signal) && (!process_inner.signal_mask.contains(signal)) {
            let mut masked = true;
            let handling_sig = process_inner.handling_sig;
            
            if handling_sig == -1 {
                // 如果目前没有正在处理的信号
                masked = false;
            } else {
                // 当前有正在处理的信号，判断这个信号的函数处理例程是否包含这个刚来的信号
                let handling_sig = handling_sig as usize;
                if !process_inner.signal_actions.table[handling_sig]
                    .mask
                    .contains(signal)
                {
                    // 函数掩码中不存在这个函数的掩码
                    masked = false;
                }
            }

            if !masked {
                // 新的信号可以执行
                drop(process_inner);
                drop(process);
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
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            process_inner.frozen = true; 
            process_inner.signals ^= SignalFlags::SIGSTOP; // 异或，也就是消除Stop这个型号，已经处理了
        }
        SignalFlags::SIGCONT => {
            if process_inner.signals.contains(SignalFlags::SIGCONT) {
                process_inner.signals ^= SignalFlags::SIGCONT;
                process_inner.frozen = false;
            }
        }
        _ => {
            // kill或者默认都是解决进程,这个状态修改大可不必，但是为了方便handle_signals()的跳出...
            process_inner.killed = true;
        }
    }
}

fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    let handler = process_inner.signal_actions.table[sig].handler;
    if handler != 0 {
        // handle不等于0，那么使用默认处理

        // 这个信号处理过了，消除他
        process_inner.handling_sig = sig as isize;
        process_inner.signals ^= signal;
        let main_task = process_inner.tasks[0].as_ref().unwrap();
        // 把trap_cx储存起来
        let mut trap_ctx = main_task.inner_exclusive_access().get_trap_cx();
        process_inner.trap_ctx_backup = Some(*trap_ctx);

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
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    process_inner.signals.check_error()
}

/// 移出无效线程
pub fn remove_inactive_task(task: Arc<TaskControlBlock>) {
    remove_task(Arc::clone(&task));
    // remove_timer(Arc::clone(&task));
}