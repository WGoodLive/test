use crate::{config::MAX_SYSCALL_NUM, task::{exit_current_and_run_next, p_block_s, suspend_current_and_run_next, TaskInfo}, timer::get_time_ms};

pub fn sys_exit(exit_code: i32) -> ! {
    println!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}


pub fn sys_get_time() -> isize{
    get_time_ms() as isize
}

/// 查询当前任务的相关信息
pub fn sys_task_info(ti:&mut TaskInfo)->isize{
    println!("current_task_id:{}\nTaskStatue:{}\ncurrent time:{},running time:{}",
    p_block_s(),"Running",get_time_ms(),get_time_ms()-ti.p_time());
    let t = ti.p_sys_time();
    for i in 0..MAX_SYSCALL_NUM{
        if(t[i]==0){
            continue;
        }
        else {
            println!("sys_id:{},number:{}",i,t[i]);
        }
    }
    0
}