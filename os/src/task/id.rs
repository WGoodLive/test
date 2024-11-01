//! - 内核栈的分配kstack_id -> 内核栈的位置
//! - 进程PidHandle -> 进程id
//! - 线程的tid -> 线程的trap_cx,用户线程栈

use core::usize;
use alloc::sync::{Arc, Weak};
use lazy_static::lazy_static;
use alloc::vec::Vec;
use crate::config::*;
use crate::mm::memory_set::MapPermission;
use crate::mm::KERNEL_SPACE;
use crate::sync::UPSafeCell;
use crate::mm::address::*;
use super::process::ProcessControlBlock;
use super::TRAMPOLINE;


lazy_static!{
    static ref PID_ALLOCATOR : UPSafeCell<RecycleAllocator> = unsafe {
        UPSafeCell::new(RecycleAllocator::new())
    };
    static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =unsafe { 
        UPSafeCell::new(RecycleAllocator::new()) 
    };
}

// 主进程
pub const IDLE_PID: usize = 0;

/// 资源分配器  
/// 感觉他的实现，跟分配物理页的usize差不多
pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}

/// 进程的唯一Pid，通过RAII思想把唯一pid绑定生命周期
pub struct PidHandle(pub usize);

pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access().alloc())
}
impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

/// 线程的tid的trap上下文地址
fn trap_cx_bottom_from_tid(tid: usize) -> usize {
    TRAP_CONTEXT_BASE - tid * PAGE_SIZE
}

/// 线程的tid的用户栈地址
fn ustack_bottom_from_tid(ustack_base: usize, tid: usize) -> usize {
    ustack_base + tid * (PAGE_SIZE + USER_STACK_SIZE)
}

///线程的 TID 、用户栈和 Trap 上下文均和线程的生命周期相同，  
/// 因此我们可以将它们打包到一起统一进行分配和回收。
pub struct TaskUserRes {
    pub tid: usize,
    pub ustack_base: usize,
    pub process: Weak<ProcessControlBlock>,
}

impl TaskUserRes {
    pub fn new(
        process:Arc<ProcessControlBlock>,
        ustack_base:usize,
        alloc_user_res:bool
    )->Self{
        let tid = process.inner_exclusive_access().alloc_tid();
        let task_user_res = Self {
            tid,
            ustack_base,
            process: Arc::downgrade(&process),
        };
        if alloc_user_res{ 
            // 用来判断TaskUserRe是否已经分配了
            // 按理说fork之后，就不需要再分配新的用户栈与数据栈
            // 用户栈位置是俺们规定的，返回的时候，更改sp
            task_user_res.alloc_user_res();
        }
        task_user_res
    }

    /// 修改tid对应的trap上下文与用户栈的地址
    pub fn alloc_user_res(&self) {
        // 因为线程拿的是进程的弱引用，要先升级
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        
        
        // 得到tid的用户栈地址，栈底
        let ustack_bottom = ustack_bottom_from_tid(self.ustack_base, self.tid);
        // tid的用户栈的栈顶
        let ustack_top = ustack_bottom + USER_STACK_SIZE;
        // 把这个用户栈当成逻辑段插入地址空间中，直接分配物理页，不是cow机制
        process_inner.memory_set.insert_framed_area(
            ustack_bottom.into(),
            ustack_top.into(),
            MapPermission::R | MapPermission::W | MapPermission::U,
        );
        
        
        // 把trap上下文写入用户空间
        let trap_cx_bottom = trap_cx_bottom_from_tid(self.tid);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        process_inner.memory_set.insert_framed_area(
            trap_cx_bottom.into(),
            trap_cx_top.into(),
            MapPermission::R | MapPermission::W,
        );
    }

    /// 回收线程的用户栈与trap上下文的资源(页表项也回收)
    fn dealloc_user_res(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        // dealloc ustack manually
        let ustack_bottom_va: VirtAddr = ustack_bottom_from_tid(self.ustack_base, self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(ustack_bottom_va.into());
        // dealloc trap_cx manually
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .remove_area_with_start_vpn(trap_cx_bottom_va.into());
    }
    /// 把线程的tid还回去
    pub fn dealloc_tid(&self) {
        let process = self.process.upgrade().unwrap();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.dealloc_tid(self.tid);
    }

    pub fn trap_cx_user_va(&self) -> usize {
        trap_cx_bottom_from_tid(self.tid)
    }

    /// 返回trap_cx的物理地址
    pub fn trap_cx_ppn(&self) -> PhysPageNum {
        let process = self.process.upgrade().unwrap();
        let process_inner = process.inner_exclusive_access();
        let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.tid).into();
        process_inner
            .memory_set
            .translate(trap_cx_bottom_va.into())// 复制vpn的页表项
            .unwrap()
            .ppn()
    }
    /// 返回内核栈的基地址
    pub fn ustack_base(&self) -> usize {
        self.ustack_base
    }
    /// 通过基地址 + tid 算内核栈的栈顶
    pub fn ustack_top(&self) -> usize {
        ustack_bottom_from_tid(self.ustack_base, self.tid) + USER_STACK_SIZE
    }
}

impl Drop for TaskUserRes {
    fn drop(&mut self) {
        self.dealloc_tid();
        self.dealloc_user_res();
    }
}

/// 内核栈与内核栈标识符有关，不与PID/TID挂钩
pub struct KernelStack(pub usize);

impl KernelStack{

    /// 把T类型的数据放在栈顶，并且返回可变引用
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }
    /// 通过kstack_id计算内核栈顶
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.0);
        kernel_stack_top
    }
}


/// 简单，就是根据kstack_id分配内核栈
pub fn kernel_stack_position(kstack_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - kstack_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/// 额，内核栈好像不用恒等映射，因为他不是代码，是数据
pub fn kstack_alloc() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.0);
        // 先取消页分配
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
        // 取消kstack_id
        KSTACK_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}