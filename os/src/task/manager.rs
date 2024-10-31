use alloc::{collections::{btree_map::BTreeMap, vec_deque::VecDeque}, sync::Arc};

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

    pub static ref PID2TCB: UPSafeCell<BTreeMap<usize,Arc<TaskControlBlock>>>=unsafe {
        UPSafeCell::new(BTreeMap::new())
    };
}

/// ARC多所有权
/// 通过ARC可以不用数据来回拷贝，数据在内核堆上
pub fn add_task(task: Arc<TaskControlBlock>) {
    PID2TCB.exclusive_access().
        insert(task.getpid(), Arc::clone(&task));
    TASK_MANAGER.exclusive_access().add(task);
}

// 获取某任务的进程控制块
pub fn pid2task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    let map = PID2TCB.exclusive_access();
    map.get(&pid).map(Arc::clone)
}

// 移出某任务的控制块
pub fn remove_from_pid2task(pid: usize) {
    let mut map = PID2TCB.exclusive_access();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}