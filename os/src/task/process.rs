use core::cell::RefMut;
use alloc::string::String;
use alloc::vec;
use alloc::{sync::{Arc, Weak}, vec::{Vec}};

use crate::fs::{Stdin, Stdout};
use crate::mm::{translated_refmut, VirtAddr};
use crate::{fs::File, mm::{MemorySet, KERNEL_SPACE}, sync::UPSafeCell, trap::{trap_handler, TrapContext}};

use super::{action::SignalActions, id::{pid_alloc, PidHandle, RecycleAllocator}, manager::{add_task, insert_into_pid2process}, signal::SignalFlags, TaskControlBlock};


/// 进程的控制块PCB
pub struct ProcessControlBlock {
    // immutable
    pub pid: PidHandle,
    // mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

impl ProcessControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// 创建进程
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        // memory_set带着trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        // 分配一个pid
        let pid_handle = pid_alloc();
        // create PCB
        let process = Arc::new(Self{
            pid:pid_handle,
            inner:unsafe {
                UPSafeCell::new(ProcessControlBlockInner{
                    is_zombie:false,
                    memory_set,
                    parent:None,
                    children:Vec::new(),
                    exit_code:0,
                    fd_table:vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals:SignalFlags::empty(),
                    tasks:Vec::new(),
                    task_res_allocator:RecycleAllocator::new(),
                    program_brk:ustack_base,
                    heap_bottom:ustack_base,
                    base_size:0,
                                        signal_mask:SignalFlags::empty(),
                    signal_actions:SignalActions::default(),
                    killed:false,
                    frozen:false,
                    handling_sig:-1,
                    trap_ctx_backup:None,
                })
            }
        });
        // 创建主线程
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        // 修改线程的trap_cx
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );
        // 主线程的trap_cx以及线程控制块已经设置完毕
        // 把主线程加入进程控制块中保存
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        drop(process_inner);
        // 把pid加入PID2PCB
        insert_into_pid2process(process.getpid(), Arc::clone(&process));
        // 把主线程加入任务管理器中
        add_task(task);
        process
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        /// 由于是clone,上下文，用户栈，跳板都被正常设置
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // 分配一个pid
        let pid = pid_alloc();
        // 复制文件描述符
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        // 创建子进程的控制块
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    program_brk:parent.program_brk,
                    heap_bottom:parent.heap_bottom,
                    base_size:parent.base_size,
                    signal_mask:SignalFlags::empty(),
                    signal_actions:SignalActions::default(),
                    killed:false,
                    frozen:false,
                    handling_sig:-1,
                    trap_ctx_backup:None,
                })
            },
        });
        parent.children.push(Arc::clone(&child));
        // 创建孩子的子进程
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            // 修改了用户栈
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_base(),
            // 没有重新分配线程的Trap_cx和用户栈
            // 但是fork的时候，复制了一份，是有Trap_Cx和用户栈的，所以不用再分配
            false,
        ));
        // 孩子进程加主线程
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        drop(child_inner);
        // 由于是复制的，但是trap_cx是中kstrack_id变化带来的内核栈有变化的，所以需要修改
        // 用户栈，上面线程控制块初始化的时候修改了
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        drop(task_inner);
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        // 把线程加入任务
        add_task(task);
        child
    }

    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        // 替换地址空间(之前的会自动回收1)
        self.inner_exclusive_access().memory_set = memory_set;
        // 地址空间换了，只需要修改线程的一些参数就行了
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();
        // push arguments on user stack
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();
        // 修改完控制块，就需要改trap上下文
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *task_inner.get_trap_cx() = trap_cx;
    }

    
}

pub struct ProcessControlBlockInner{
    pub is_zombie:bool,
    pub memory_set:MemorySet,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub exit_code: i32,
    // 实现File + Send + Sync的结构体
    // Option使得我们可以区分一个文件描述符当前是否空闲，当它是 None 的时候是空闲的
    // Arc:可能会有多个进程共享同一个文件对它进行读写。此外被它包裹的内容会被放到内核堆而不是栈上,编译的时候不用固定大小
    pub fd_table:Vec<Option<Arc<dyn File + Send + Sync>>>,
    pub signals:SignalFlags,
    // 线程存放
    pub tasks:Vec<Option<Arc<TaskControlBlock>>>,
    // 给线程分配资源的通用分配器
    pub task_res_allocator: RecycleAllocator,
    // 堆顶
    pub program_brk:usize,
    // 堆底
    pub heap_bottom:usize,
    // 应用数据大小
    pub base_size:usize,
    // 有线程之后，信号不好加了
    pub signal_mask:SignalFlags,
    // 进程的函数例程
    pub signal_actions:SignalActions,
    // 进程是否被杀死，不是是否被捕获
    pub killed:bool,
    // 进程收到SIGSTOP然后被暂停执行，等待SIGCONT
    pub frozen:bool,
    // 正在处理的信号
    pub handling_sig:isize,
    // 处理信号的时候，储存trap_cx
    pub trap_ctx_backup: Option<TrapContext>,

}

impl ProcessControlBlockInner {
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }

    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }

    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }

    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
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
}