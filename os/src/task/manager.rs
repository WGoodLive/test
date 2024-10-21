use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::sync::UPSafeCell;

use super::TaskControlBlock;
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
}

lazy_static! {
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> = unsafe {
        UPSafeCell::new(TaskManager::new())
    };
}

/// ARC多所有权
/// 通过ARC可以不用数据来回拷贝，数据在内核堆上
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}


pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}