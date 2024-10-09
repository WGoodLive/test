mod context;
mod switch;
#[allow(clippy::module_inception)] // 允许跳过重复警告
mod task;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::config::*;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use log::warn;
use switch::__switch;
use crate::loader::*;
use context::*;
use task::*;
pub struct TaskManager{
    num_app:usize,
    inner:UPSafeCell<TaskManagerInner>, // 这里字段让TaskManager成为Sync
}

struct TaskManagerInner{
    tasks: Vec<TaskControlBlock>,// [TaskControlBlock; MAX_APP_NUM], // 每个任务的控制块
    current_task: usize, // 当前任务id
}

impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }


    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn run_next_task(&self){ 
        if let Some(next) = self.find_next_task(){
            // 如果find_next_task返回类型是Some，执行下面代码
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;

            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;

            // 这里不要使用ref，因为 这里有个as强制约束，加ref需要多行代码
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
        }
        else {
            warn!("All applications completed!");
            shutdown(false);
        }

    }

    /// 返回下一个task的id
    fn find_next_task(&self) -> Option<usize>{
        let inner = self.inner.exclusive_access(); 
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1) 
        // 这步很重要，相当于从这个app向后找，并且保证全覆盖
        .map(|id| id % self.num_app)
        .find(|id|inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];

        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();

        unsafe {
            __switch((&mut _unused) as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task");
    }

    fn get_current_token(&self) -> usize {

        let inner = self.inner.exclusive_access();

        let current = inner.current_task;

        inner.tasks[current].get_user_token()

    }


    fn get_current_trap_cx(&self) -> &mut TrapContext {

        let inner = self.inner.exclusive_access();

        let current = inner.current_task;

        inner.tasks[current].get_trap_cx()

    }

    fn change_current_program_brk(&self,size:i32) -> Option<usize>{
        let mut inner = self.inner.exclusive_access();
        let current_id = inner.current_task;
        inner.tasks[current_id].change_program_brk(size)
    }

    fn add_current_share_page(&self,id:usize,type_page:bool)->Option<usize>{
        let mut inner = self.inner.exclusive_access();
        let current_id = inner.current_task;
        inner.tasks[current_id].add_program_share_page(id,type_page)
    }

    
}



lazy_static!{
    pub static ref TASK_MANAGER:TaskManager={
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks:Vec<TaskControlBlock> = Vec::new();

        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(
                get_app_data(i),
                i,
            ));
        }

        TaskManager {
            num_app,
            inner: unsafe { UPSafeCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            })},
        }
      
    };
    
}


pub fn suspend_current_and_run_next(){
    mark_current_suspended();
    run_next_task();
}

pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

pub fn run_first_task(){
    TASK_MANAGER.run_first_task();
}

fn mark_current_suspended(){
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited(){
    TASK_MANAGER.mark_current_exited();
}


fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

pub fn current_user_token() -> usize {

    TASK_MANAGER.get_current_token()

}


pub fn current_trap_cx() -> &'static mut TrapContext {

    TASK_MANAGER.get_current_trap_cx()

}

// 因为要修改运行的进程的断点，所以需要提供一个外部接口
pub fn change_program_sbrk(size:i32) -> Option<usize>{
    TASK_MANAGER.change_current_program_brk(size)
}

/// 系统调用的公共接口
pub fn add_share_page(id:usize) -> Option<usize>{
    TASK_MANAGER.add_current_program_share_page(id)
}