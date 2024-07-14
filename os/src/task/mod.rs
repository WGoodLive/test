mod context;
mod switch;
#[allow(clippy::module_inception)] // 允许跳过重复警告
mod task;
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::config::*;
use crate::timer::get_time_ms;
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

/// 任务信息
/// - status:任务状态
/// - syscall_times:系统调用次数
/// - time:运行时间（包括等待时间）
#[derive(Copy,Clone)]
pub struct TaskInfo{
    status: TaskStatus,
    syscall_times: [u32; MAX_SYSCALL_NUM],
    time: usize
}


impl TaskInfo {
    // pub fn init() ->Self{
    //     Self{
    //         status:TaskStatus::Ready,
    //         time:0,
    //         syscall_times:[0;MAX_SYSCALL_NUM],
    //     }
    // }
    pub fn p_status(&self) ->TaskStatus{
        self.status
    }
    pub fn p_time(&self) -> usize{
        self.time
    }
    pub fn p_sys_time(&mut self)-> &mut [u32]{
        &mut self.syscall_times
    }
}

struct TaskManagerInner{
    tasks: [TaskControlBlock; MAX_APP_NUM], // 每个任务的控制块
    current_task: usize, // 当前任务id
}

pub struct TaskInfoArr{
    current_task_id:usize,
    data:[TaskInfo;MAX_APP_NUM],
}

impl TaskInfoArr {
    pub fn add_times(&mut self,sys_id:usize)->isize{
        let id = self.current_task_id;
        let it: &mut TaskInfo = &mut self.data[id];
        let mut arr = it.syscall_times;
        arr[sys_id]+=1;
        0
    }

    pub fn start(&mut self)->isize{
        let id = self.current_task_id;
        let it: &mut TaskInfo = &mut self.data[id];
        if(it.status != TaskStatus::Running){
            it.time = get_time_ms();
        }
        it.status = TaskStatus::Running;
        0
    }

    pub fn yy(&mut self)->&mut TaskInfo{
        &mut self.data[self.current_task_id]
    }
}

pub static mut TASKINFOARR:TaskInfoArr = {
    TaskInfoArr{
        current_task_id : 0,
        data:[TaskInfo{
            status:TaskStatus::Ready,
            time:0,
            syscall_times:[0;MAX_SYSCALL_NUM],
        };MAX_APP_NUM],
    }
};

impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }


    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        println!("id:{},end time:{}",current,get_time_ms());

        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn run_next_task(&self){ 
        println!("switch...");
        if let Some(next) = self.find_next_task(){
            // 如果find_next_task返回类型是Some，执行下面代码
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            
            &mut inner.tasks[next].task_cx.set_s(0, next);
            // 这里不要使用ref，因为 这里有个as强制约束，加ref需要多行代码
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            unsafe {
                TASKINFOARR.current_task_id = next;
                TASKINFOARR.start(); // 开始计时
                // println!("id:{},start time:{}",next,TASKINFOARR.data[next].time);
            }
            
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
        unsafe {
            TASKINFOARR.current_task_id = 0;
            TASKINFOARR.start(); // 开始计时
            // println!("id:{},start time:{}",0,TASKINFOARR.data[0].time);
        }
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner); // inner如果不drop,另个应用借不了
        let mut _unused = TaskContext::zero_init();

        unsafe {
            __switch((&mut _unused) as *mut TaskContext, next_task_cx_ptr);
        }
        
        panic!("unreachable in run_first_task");
    }
}



lazy_static!{
    pub static ref TASK_MANAGER:TaskManager={
        let num_app:usize = get_num_app();
        let mut tasks = [
            TaskControlBlock{
                task_status:TaskStatus::UnInit,
                task_cx:TaskContext::zero_init(),
            };
            MAX_APP_NUM
        ];
        
        /* 
        or (i, task) in tasks.iter_mut().enumerate() {
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
        }
        */
        for i in 0..num_app{ // 软件先初始化
            tasks[i] = TaskControlBlock{
                task_status:TaskStatus::Ready,
                task_cx:TaskContext::goto_restore(init_app_cx(i)),
            };

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

pub fn p_block_s() ->usize{
    let t = TASK_MANAGER.inner.exclusive_access();
    let current = t.current_task;
    t.tasks[current].task_cx.p_s(0)
}
