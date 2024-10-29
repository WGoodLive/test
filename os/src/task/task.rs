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
use alloc::vec;
use alloc::{sync::{Arc, Weak}, task, vec::{Vec}};


use crate::fs::{Stdin, Stdout};
use crate::{fs::File, mm::{address::{PhysPageNum, VirtAddr}, memory_set::{MapPermission, MemorySet}, KERNEL_SPACE}, sync::UPSafeCell, trap::{trap_handler, TrapContext}};

use super::{context::TaskContext, pid::{pid_alloc, KernelStack, PidHandle}, TRAP_CONTEXT};

/// 任务控制块(很重要)
pub struct TaskControlBlockInner{
    pub task_status: TaskStatus,// 任务状态
    pub task_cx: TaskContext,
    // memory_sret的？物理？地址(包含页面映射）
    pub memory_set:MemorySet,
    // 物理地址
    pub trap_cx_ppn:PhysPageNum,
    // 应用数据大小
    pub base_size:usize,
    pub heap_bottom:usize,
    pub program_brk:usize,
    
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
    // 实现File + Send + Sync的结构体
    // Option使得我们可以区分一个文件描述符当前是否空闲，当它是 None 的时候是空闲的
    // Arc:可能会有多个进程共享同一个文件对它进行读写。此外被它包裹的内容会被放到内核堆而不是栈上,编译的时候不用固定大小
    pub fd_table:Vec<Option<Arc<dyn File + Send + Sync>>> 
}

pub struct TaskControlBlock{
    // immutable
    pub pid:PidHandle,
    pub kernel_stack:KernelStack,
    // // mutable
    inner:UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock{
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn getpid(&self) -> usize{
        self.pid.0
    }

    pub fn new(elf_data: &[u8]) -> Self {
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set.translate(
            VirtAddr::from(TRAP_CONTEXT).into() // riscv把虚拟地址上下文存在这个顶端
        ).unwrap().ppn();

        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        let task_control_block = Self{
            pid:pid_handle,
            kernel_stack,
            inner:unsafe {
                UPSafeCell::new({TaskControlBlockInner{
                    task_status:  TaskStatus::Ready,// 任务状态
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    memory_set:memory_set,
                    trap_cx_ppn,
                    base_size:user_sp,
                    heap_bottom:user_sp,
                    program_brk:user_sp,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table:vec![
                        // 标准输入
                        Some(Arc::new(Stdin)),
                        // 标准输出
                        Some(Arc::new(Stdout)),
                        // 当程序执行过程中发生错误时，错误信息会被发送到stderr流中显示在屏幕，然后Stdout仍然被别的进程使用
                        Some(Arc::new(Stdout))
                    ],
                }})      
            }
        };

        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point, 
            user_sp, 
            KERNEL_SPACE.exclusive_access().token(), 
            kernel_stack_top, 
            trap_handler as usize
        );
        
        task_control_block
    }
    pub fn exec(&self, elf_data: &[u8]) {
        // 获取子程序的Elf信息
        // from_elf是自己封装的方法，所以可以提取出想要的信息
        // memory_set是含有satp与真实物理页的
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_pnn = memory_set.translate( // 复制vpn的页表项
            VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap().ppn();
        let mut inner = self.inner_exclusive_access();
        inner.memory_set = memory_set; // 旧的memory_set会被释放
        inner.trap_cx_ppn =  trap_cx_pnn;

        let trap_cx = inner.get_trap_cx();
        // 因为这个trap_cx是我们定的，所以需要我们在里面增加内容
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        
        
    }
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        let mut parent_inner = self.inner_exclusive_access();

        // 实现逻辑地址的复制
        let memory_set = MemorySet::from_existed_user(
            &parent_inner.memory_set
        );
        // 获取trap上下文
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }

        let task_control_block = Arc::new(TaskControlBlock{
            pid:pid_handle,
            kernel_stack,
            inner:unsafe {
                UPSafeCell::new(TaskControlBlockInner{
                    // 用于进程进入内核，所以需要保存satp什么的
                    trap_cx_ppn,
                    task_status:TaskStatus::Ready,
                    // 这个储存的任务ra,sp,寄存器，不需要satp等信息，进程切换用
                    task_cx:TaskContext::goto_trap_return(kernel_stack_top),
                    memory_set,
                    base_size:parent_inner.base_size,
                    heap_bottom:parent_inner.heap_bottom,
                    program_brk:parent_inner.program_brk,
                    parent:Some(Arc::downgrade(self)),
                    children:Vec::new(),
                    exit_code:0,
                    fd_table:new_fd_table,
                })
            }
        });
        // Arc指针的clone是指针，强引用
        parent_inner.children.push(task_control_block.clone()); // 这个任务控制块后面信息没发实现同步改变啊

        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();

        // trapcontxt被当逻辑段复制了        
        trap_cx.kernel_sp = kernel_stack_top; // 不同应用，内核栈不同
        task_control_block
    }
    
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    fn get_status(&self) -> TaskStatus{
        self.task_status
    }

    pub fn is_zombie(&self) -> bool{
        self.get_status() == TaskStatus::Zombie
    }


    pub fn change_program_brk(&mut self,size:i32) ->Option<usize>{
        let old_brk = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        // 不能出堆的范围
        if new_brk < self.heap_bottom as isize{
            return None;
        }

        let result = if size<0{
            self.memory_set
                .shrink_to(VirtAddr(self.heap_bottom),VirtAddr(new_brk as usize))
        }else {
            self.memory_set
                .append_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        };

        if result{
            self.program_brk = new_brk as usize;
            Some(old_brk)
        }else {
            None
        }
    }

    /// 分配文件描述符，也就是对应用对自己文件的别名
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
    
}

