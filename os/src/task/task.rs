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
}

use alloc::task;
use riscv::register::sie;

use crate::{mm::{address::{PhysPageNum, VirtAddr}, memory_set::{MapPermission, MemorySet}, KERNEL_SPACE}, trap::{trap_handler, TrapContext}};

use super::{context::TaskContext, kernel_stack_position, TRAP_CONTEXT};

/// 任务控制块(很重要)
pub struct TaskControlBlock{
    pub task_status: TaskStatus,// 任务状态
    pub task_cx: TaskContext,
    // memory_sret的？物理？地址(包含页面映射）
    pub memory_set:MemorySet,
    // 物理地址
    pub trap_cx_ppn:PhysPageNum,
    // 应用数据大小
    pub base_size:usize,

    pub heap_bottom:usize,
    pub program_brk:usize
}


impl TaskControlBlock {

    /// 堆分配
    


    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn new(elf_data:&[u8],app_id:usize) ->Self{
        let (memory_set,user_sp,entry_point)=MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
        .translate(VirtAddr::from(TRAP_CONTEXT).into())
        .unwrap()
        .ppn();
        let task_status = TaskStatus::Ready;
        let (kernel_stack_bottom,kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access()
        .insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W
        );
        
        let task_control_block = Self{
            task_status,
            task_cx:TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size:user_sp,
            heap_bottom:user_sp,
            program_brk:user_sp,
        };

        let trap_cx = task_control_block.get_trap_cx(); 
        // 此时没有特别明显的用户了，内核可以修改任意物理页帧数据，就想成C语言，riscv内核应该可以读写任意内存
        // 这里对裸指针解引用成立的原因在于：当前已经进入了内核地址空间，而要操作的内核栈也是在内核地址空间中的?????
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
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

    pub fn add_program_share_page(&mut self,id:usize,type_add:bool) ->Option<usize>{
        let mut size = crate::config::PAGE_SIZE as isize;
        if(!type_add){
            size *=-1;
        }
        let old_brk = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        // 不能出堆的范围
        if new_brk < self.heap_bottom as isize{
            return None;
        }

        let result = if size<0{
            self.memory_set
                .shrink_share_page(VirtAddr(self.heap_bottom),VirtAddr(new_brk as usize),id)
        }else {
            self.memory_set
                .append_share_page(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize),id)
        };

        if result{
            self.program_brk = new_brk as usize;
            Some(old_brk)
        }else { 
            None
        }
    }
    
}

