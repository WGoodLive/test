//! 当前的调度单位...不是进程控制块，麻了
use alloc::{sync::Arc};

use super::{manager::fetch_task, process::ProcessControlBlock, signal::SignalFlags, switch::__switch, TaskControlBlock, TaskStatus};
use crate::{sync::UPSafeCell, task::TaskContext, trap::TrapContext};
use lazy_static::lazy_static;
/// 保存当前的进程信息
/// - current:当前任务的任务控制块
/// - idle_task_cx: idle 控制流的任务上下文
pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}




impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take() // take():取出Option里面的元素，获取所有权，原来的地方为None
    }

    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }

    fn get_idle_task_cx_ptr(&mut self)  -> *mut TaskContext{
        &mut self.idle_task_cx as &mut _
    }

    fn change_current_program_brk(&self,size:i32) -> Option<usize>{
        let task = self.current().unwrap();
        let process = task.process.upgrade().unwrap();
        let mut inner = process.inner_exclusive_access();
        inner.change_program_brk(size)
    }
}

pub fn change_program_sbrk(size:i32)->Option<usize>{
    PROCESSOR.exclusive_access().change_current_program_brk(size)
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}


pub fn current_user_token() -> usize {
    let process = current_process();
    let token = process.inner_exclusive_access().get_user_token();
    token
}

pub fn current_process() -> Arc<ProcessControlBlock> {
    current_task().unwrap().process.upgrade().unwrap()
}

/// 返回trap上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().inner_exclusive_access().get_trap_cx()
}
/// 返回线程的trap上下文，在进程地址空间的虚拟地址
pub fn current_trap_cx_user_va() -> usize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .trap_cx_user_va()
}


pub fn run_tasks(){
    loop{
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() { // 从准备好的任务中，弹出一个
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            processor.current = Some(task);
            drop(processor);
            unsafe {
                // 导致可能task_inner在换之前没有被回收，导致UPSafeCell的报错！！！
                __switch(
                    idle_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
        }
    }
}

pub fn schedule(switched_task_cx_ptr:*mut TaskContext){
    let mut processor =PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(
            switched_task_cx_ptr,
            idle_task_cx_ptr,
        );
    }
}



lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe {
        UPSafeCell::new(Processor::new())
    };
}