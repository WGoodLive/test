use alloc::sync::Arc;

use crate::{loader::get_app_data_by_name, mm::page_table::{translated_refmut, translated_str}, task::{exit_current_and_run_next, manager::add_task, processor::{change_program_sbrk, current_task, current_user_token}, suspend_current_and_run_next}, timer::get_time_ms};

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

pub fn sys_exec(path:*const u8)-> isize{
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()){
        let task = current_task().unwrap();
        task.exec(data); // trap返回的程序变了，这个返回值意义不大了
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

pub fn sys_sbrk(size:i32) -> isize{
    if let Some(old_brk) = change_program_sbrk(size){
        old_brk as isize
    }
    else{
        -1
    }
}
