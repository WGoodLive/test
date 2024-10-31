use crate::fs::{open_file, OpenFlags};
use crate::mm::{translated_ref, translated_refmut, translated_str};
use crate::task::action::SignalAction;
use crate::task::manager::pid2task;
use crate::task::signal::{SignalFlags, MAX_SIG};
use crate::task::{
    manager::add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
};
use crate::timer::get_time_ms;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}


pub fn sys_fork()->isize{
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    
    let new_pid = new_task.pid.0;

    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();

    trap_cx.x[10] = 0; // 由于new_task是子程序，返回值为0
    add_task(new_task);
    new_pid as isize // x[10]返回值为new_pid
}

pub fn sys_exec(path:*const u8,mut args: *const usize)-> isize{
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec :Vec<String> = Vec::new();
    loop{
        let arg_str_ptr = *translated_ref(token, args);// args至少有一个元素0
        if arg_str_ptr==0{
            break;
        }else{
            args_vec.push(translated_str(token, arg_str_ptr as *const u8));
            unsafe {
                args = args.add(1);
            }
        }
    }
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) { 
        // 通过只读代码，就可以返回了，不会覆盖文件
        // 这里将代码(文件驱动的内存那地方)与数据(DRAM的区域) 解耦合了！！！
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice(),args_vec);// trap返回的程序变了，这个返回值意义不大了
        0
    }else{
        -1
    }
}

pub fn sys_waitpid(pid:isize,exit_code_ptr:*mut i32)-> isize{
    let task = current_task().unwrap();

    let mut inner = task.inner_exclusive_access();
    if inner.children
    .iter()
    .find(|p|{pid==-1 || pid as usize == p.getpid()})
    .is_none(){
        return -1; // 没有找到目标孩子
    }
    let pair = inner.children
    .iter()
    .enumerate()
    .find(|(_,p)|{
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
    });
    if let Some((idx,_)) = pair{
        let child = inner.children.remove(idx);
        // fork的时候返回的强引用，也被加入执行队列了;另一个强引用是父亲储存着
        // 在这里父亲想要回收孩子，前提是孩子已经变成了僵尸进程，也就是执行队列里没有这个强引用
        
        assert_eq!(Arc::strong_count(&child), 1); 
        
        let found_pid = child.getpid();
        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
        // 上面通过取出child的所有权，然后这个函数结束，开始回收RAII方法的页表机制，以及该进程的内核栈
    }else{
        -2
    }
}

pub fn sys_get_time() -> isize{
    get_time_ms() as isize
}
pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}


/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针（SignalAction 结构稍后介绍）。
/// 返回值：如果传入参数错误（比如传入的 action 或 old_action 为空指针或者）
/// 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
pub fn sys_sigaction(
    signum: i32, // 信号编号
    action: *const SignalAction, // action 表示要设置成的处理函数的指针
    old_action: *mut SignalAction, // 保存设置之前的处理函数的指针
) -> isize{
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if signum as usize > MAX_SIG{
        return  -1;
    }
    // 通过from_bit获得signum相应的信号编号
    if let Some(flag) = SignalFlags::from_bits(1<<signum){
        if check_sigaction_error(flag, action as usize, old_action as usize){
            return -1;
        }else{
            let prev_action = inner.signal_actions.table[signum as usize];
            // 将之前的 函数例程 放在用户内存中
            *translated_refmut(token, old_action) = prev_action;
            // 内核空间保存新的函数例程
            inner.signal_actions.table[signum as usize] = *translated_ref(token, action);
            0
        }
    }else {
        -1
    }
}

pub fn sys_sigprocmask(mask: u32) -> isize {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        let old_mask = inner.signal_mask;
        if let Some(flag) = SignalFlags::from_bits(mask) {
            inner.signal_mask = flag;
            old_mask.bits() as isize
        } else {
            -1
        }
    } else {
        -1
    }
}

// 向进程pid加入signum信号
pub fn sys_kill(pid:usize,signum: i32)->isize{
    if let Some(task) = pid2task(pid) {
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            let mut task_ref = task.inner_exclusive_access();
            // 如果任务已经存在未解决的信号，不覆盖，添加失败
            // 比如多个子进程kill父进程自己状态变了
            if task_ref.signals.contains(flag) {
                return -1;
            }
            // 否则加入这个信号量SignalFlags
            task_ref.signals.insert(flag);
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// 检查函数例程设置是否合理
/// - 服务例程没有空指针
/// - SIGKILL和SIGSTOP不允许用户设置处理例程，内核自己设置
fn check_sigaction_error(signal: SignalFlags, action: usize, old_action: usize) -> bool {
    if action == 0 
        || old_action == 0
        || signal == SignalFlags::SIGKILL
        || signal == SignalFlags::SIGSTOP
    {
        true
    } else {
        false
    }
}

/* 
这个是用户态的 
Option
引用，不用裸指针

pub fn sigaction(
    signum: i32,
    action: Option<&SignalAction>,
    old_action: Option<&mut SignalAction>,
) -> isize;
*/

/// 用户自己调用，用户例程执行结束，退出信号处理，返回用户
/// 如果用户没有调用这个,exit会收到什么参数？
pub fn sys_sigreturn() -> isize {
    if let Some(task) = current_task() {
        let mut inner = task.inner_exclusive_access();
        inner.handling_sig = -1; // 当前没有信号处理例程
        // 获取上下文可变引用
        let trap_ctx = inner.get_trap_cx();
        // 修改上下文应用
        *trap_ctx = inner.trap_ctx_backup.unwrap();
        // 函数返回参数,
        // 错：由于函数没有接受之前传的参数，所以这个参数是 信号，不是信号编号(这个是信号处理之后的上下文，这个返回值没用到)
        // 对：信号处理之前，保存之前系统调用返回值，会不断迭代回最初没信号的时候的返回值
        // 不理解这里trap_cx不会被覆盖啊？会的，因为syscall的值会给trap_cx[10],我们要提前把syscall返回值修改了
        trap_ctx.x[10] as isize
    } else {
        -1
    }
}
