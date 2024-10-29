use core::usize;
use lazy_static::lazy_static;
use alloc::vec::Vec;
use crate::config::*;
use crate::mm::memory_set::MapPermission;
use crate::mm::KERNEL_SPACE;
use crate::sync::UPSafeCell;
use crate::mm::address::*;
use super::TRAMPOLINE;

 
/// 进程的唯一Pid，通过RAII思想把唯一pid绑定生命周期
pub struct PidHandle(pub usize);

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}


// 目前没有绑定进程与pid
struct PidAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl PidAllocator {
    pub fn new()->Self{
        PidAllocator { current: 0, recycled: Vec::new() }
    }

    pub fn alloc(&mut self) -> PidHandle{
        if let Some(pid) = self.recycled.pop(){
            PidHandle(pid)
        }else{
            self.current +=1;
            PidHandle(self.current-1)
        }
    }

    pub fn dealloc(&mut self,pid:usize){
        assert!(pid < self.current);
        assert!(self.recycled.iter().find(|ppid|**ppid==pid).is_none(),"pid {} has been deallocated!", pid);
        self.recycled.push(pid);
    }
}

lazy_static!{
    static ref PID_ALLOCATOR : UPSafeCell<PidAllocator> = unsafe {
        UPSafeCell::new(PidAllocator::new())
    };
}

pub fn pid_alloc() -> PidHandle {
    PID_ALLOCATOR.exclusive_access().alloc()
}

/// 原来访问内核栈需要通过意义不大的编号
/// 现在通过Pid就可以通过封装的方法访问他专属的内核栈
pub struct KernelStack{
    pid:usize
}

// 任何一个内核栈都是一个小的逻辑段
impl KernelStack { // 这个没有加入用户的逻辑段中，所以fock不会复制他
    pub fn new(pid_handle:&PidHandle) -> Self{
        let pid = pid_handle.0;
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(pid);
        KERNEL_SPACE
            .exclusive_access()
            .insert_framed_area( 
                // 不是恒等插入，随机插入，恒等插入只有内核执行代码需要恒等，数据不需要平滑过渡
                // (跳板只需要保证所有该代码映射同一页)
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            );
    KernelStack{
        pid:pid_handle.0,
        }
    }

    pub fn push_on_top<T>(&self,value:T) -> *mut T where T:Sized,{ // Sized必须可以实例化，有固定大小
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe { *ptr_mut = value; }
        ptr_mut
    }

    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.pid);
        kernel_stack_top
    }


}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.pid);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}

pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}



