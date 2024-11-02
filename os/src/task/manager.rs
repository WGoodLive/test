//！ 调度队列中，接下来调用什么任务
//!  各个进程的  进程控制块
use alloc::{collections::{btree_map::BTreeMap, vec_deque::VecDeque}, sync::Arc};

use crate::sync::UPSafeCell;

use super::{process::ProcessControlBlock, TaskControlBlock};
use lazy_static::lazy_static;

/// 原来任务管理器不仅管理任务，还维护当前CPU执行的任务  
/// 接下来我们让任务管理器仅管理任务
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.双端队列
impl TaskManager{
    pub fn new()->Self{
        Self { ready_queue: VecDeque::new(), }
    }

    /// 添加进程
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }

    /// 获取进程（先进先出）
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
    /// 移出任务/线程
    pub fn remove(&mut self, task: Arc<TaskControlBlock>) {
        if let Some((id, _)) = self
            .ready_queue
            .iter()
            .enumerate()
            .find(|(_, t)| Arc::as_ptr(t) == Arc::as_ptr(&task))
            // 两个任务控制块的指向一样
        {
            self.ready_queue.remove(id);
        }
    }
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };

    pub static ref PID2PCB: UPSafeCell<BTreeMap<usize,Arc<ProcessControlBlock>>>=unsafe {
        UPSafeCell::new(BTreeMap::new())
    };
}

/// ARC多所有权
/// 通过ARC可以不用数据来回拷贝，数据在内核堆上
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

pub fn remove_task(task:Arc<TaskControlBlock>){
    TASK_MANAGER.exclusive_access().remove(task)
}


// 获取某任务的进程控制块
pub fn pid2process(pid: usize) -> Option<Arc<ProcessControlBlock>> {
    let map = PID2PCB.exclusive_access();
    map.get(&pid).map(Arc::clone)
}

/// 移出某个pid对进程控制块的映射
pub fn remove_from_pid2process(pid: usize) {
    let mut map = PID2PCB.exclusive_access();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}

/// 插入某个pid对进程控制块的映射
pub fn insert_into_pid2process(pid: usize, process: Arc<ProcessControlBlock>) {
    PID2PCB.exclusive_access().insert(pid, process);
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}

/// 唤醒任务
pub fn wakeup_task(task: Arc<TaskControlBlock>) {
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    add_task(task);
}