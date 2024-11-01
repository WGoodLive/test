//! 在操作系统看来，任务 = 调度单位 = 线程，  
//! 进程就是个容器

/* 
通过 #[derive(...)] 可以让编译器为你的类型提供一些 Trait 的默认实现。
    实现了 Clone Trait 之后就可以调用 clone 函数完成拷贝；
    实现了 PartialEq Trait 之后就可以使用 == 运算符比较该类型的两个实例，从逻辑上说只有 两个相等的应用执行状态才会被判为相等，而事实上也确实如此。
    Copy 是一个标记 Trait，决定该类型在按值传参/赋值的时候采用移动语义还是复制语义。
*/

#[derive(Clone, Copy,PartialEq)]
/// 任务状态
pub enum TaskStatus{
    UnInit, // 未初始化
    Ready, // 准备运行
    Running, // 正在运行
    Exited, // 已退出
    Zombie, // 僵尸进程
}

use core::cell::RefMut;
use alloc::string::String;
use alloc::vec;
use alloc::{sync::{Arc, Weak}, task, vec::{Vec}};


use crate::fs::{Stdin, Stdout};
use crate::mm::translated_refmut;
use crate::{fs::File, mm::{address::{PhysPageNum, VirtAddr}, memory_set::{MapPermission, MemorySet}, KERNEL_SPACE}, sync::UPSafeCell, trap::{trap_handler, TrapContext}};

use super::action::SignalActions;
use super::id::{kstack_alloc, TaskUserRes};
use super::process::ProcessControlBlock;
use super::signal::SignalFlags;
use super::{context::TaskContext, id::{pid_alloc, KernelStack, PidHandle}, TRAP_CONTEXT};

/// 任务控制块(很重要)
pub struct TaskControlBlockInner{
    pub res: Option<TaskUserRes>, // 线程的tid,trap_cx,userstack
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub trap_cx_ppn:PhysPageNum,
    pub exit_code: Option<i32>,


}

pub struct TaskControlBlock{
    // immutable
    pub process:Weak<ProcessControlBlock>,
    pub kstack:KernelStack, // 内核栈
    // // mutable
    inner:UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock{
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn gettid(&self) -> usize{
        self.inner_exclusive_access().res.as_ref().unwrap().tid
    }

    /// 创建线程
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_base: usize,
        alloc_user_res: bool, // 如果实现分配了trap_cx与用户栈,此参数为true
    ) -> Self {
        // 创建一个线程的TaskUserRes
        let res = TaskUserRes::new(Arc::clone(&process), ustack_base, alloc_user_res);
        // 获取trap_cx的物理地址
        let trap_cx_ppn = res.trap_cx_ppn();
        // 获得内核的空间，然后得到内核的kstack_id
        let kstack = kstack_alloc();
        // 返回tip的内核栈顶
        let kstack_top = kstack.get_top();
        Self {
            process: Arc::downgrade(&process),
            kstack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,

                })
            },
        }
    }
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }


    fn get_status(&self) -> TaskStatus{
        self.task_status
    }

    pub fn is_zombie(&self) -> bool{
        self.get_status() == TaskStatus::Zombie
    }
    
}

