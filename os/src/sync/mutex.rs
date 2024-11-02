use alloc::{collections::vec_deque::VecDeque, sync::Arc};

use crate::task::{block_current_and_run_next, current_task, manager::wakeup_task, suspend_current_and_run_next, TaskControlBlock};

use super::UPSafeCell;


pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

/// 基于yield
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        // locked = true的时候会返回，执行临界区；否则会yield
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                // 防止来回自由无条件进入
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// 基于阻塞
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}


/// 需要保存阻塞的队列
pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task); // 唤醒的线程是初始化的falsw
        } else {
            mutex_inner.locked = false; // 如果没有可唤醒的线程，就让自己是false，可以直接进入临界区
        }
    }
}