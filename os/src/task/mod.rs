mod context;
mod switch;
#[allow(clippy::module_inception)] // 允许跳过重复警告
mod task;
mod pid;

use crate::config::*;
use alloc::sync::Arc;
use lazy_static::*;
use manager::add_task;
use processor::{schedule, take_current_task};
use crate::loader::*;
use context::*;
use task::*;
pub mod manager;
pub mod processor;


lazy_static!{
    pub static ref INITPROC:Arc<TaskControlBlock> = Arc::new(
        TaskControlBlock::new(get_app_data_by_name("initproc").unwrap())
    );
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
