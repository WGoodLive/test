use alloc::sync::Arc;

use crate::{mm::kernel_token, task::{manager::add_task, TaskControlBlock}, trap::{trap_handler, TrapContext}};

use super::current_task;



/// 功能：当前进程创建一个新的线程
/// 参数：entry 表示线程的入口函数地址，arg 表示传给线程入口函数参数
/// 返回值：创建的线程的 TID
/// syscall ID: 1000
pub fn sys_thread_create(entry: usize, arg: usize) -> isize{
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    // 创建一个线程(由于allic_user_res为true，会自动申请用户栈和trap_cx)
    let new_task = Arc::new(TaskControlBlock::new(
        Arc::clone(&process),
        task.inner_exclusive_access().res.as_ref().unwrap().ustack_base,
        true,
    ));
    // 把任务加进任务调度器
    add_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // 把任务加入进进程管理
    let tasks = &mut process_inner.tasks;
    // 保证tid对应tasks的索引，如果有缺失，加None补位
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));
    // 修改新的任务控制块的trap_cx
    let new_task_trap_cx = new_task_inner.get_trap_cx();
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.ustack_top(),
        kernel_token(),
        new_task.kstack.get_top(),
        trap_handler as usize,
    );
    // 由于参数要加到线程里面，所以要实现参数覆盖
    // 这里不用保存主线程的trap_backup,因为主线程有自己的trap_cx和kstack
    (*new_task_trap_cx).x[10] = arg;
    new_task_tid as isize
}


/// 功能：返回线程的tid(线程标识zhi符)
/// syscall ID:1001
pub fn sys_gettid() -> isize{
    current_task()
    .unwrap()
    .inner_exclusive_access()
    .res
    .as_ref()
    .unwrap()
    .tid as isize
}

/// 功能：等待当前进程内的一个指定(非主线程)线程退出
/// 参数：tid 表示指定线程的 TID
/// 返回值：如果线程不存在，返回-1；如果线程还没退出，返回-2；其他情况下，返回结束线程的退出码
/// syscall ID: 1002
/// 疑问：等退出的时候，不用换任务？一直等//等一下，如果没有执行其他，也可以一直等while
pub fn sys_waittid(tid: usize) -> i32{
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task_inner = task.inner_exclusive_access();
    let mut process_inner = process.inner_exclusive_access();
    // 线程不能回收自己
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;
    let waited_task = process_inner.tasks[tid].as_ref();
    if let Some(waited_task) = waited_task {
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().exit_code {
            exit_code = Some(waited_exit_code);
        }
    } else {
        // waited thread does not exist
        return -1;
    }
    if let Some(exit_code) = exit_code {
        // 此时的内核栈没了，因为RAII
        process_inner.tasks[tid] = None;
        exit_code
    } else {
        // waited thread has not exited
        -2
    }
}