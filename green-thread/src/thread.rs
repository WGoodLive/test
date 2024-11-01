use crate::RUNTIME;

pub const DEFAULT_STACK_SIZE:usize = 0x1000;
pub const MAX_TASKS:usize = 6;

/// 线程切换/调度上下文
struct TaskContext{
    x1:u64, // 当前的PC
    x2:u64, // sp
    x8:u64, // s0,fp
    x9:u64, // s1
    x18:u64,// s2-s11
    x19:u64,
    x20:u64,
    x21:u64,
    x22:u64,
    x23:u64,
    x24:u64,
    x25:u64,
    x26:u64,
    x27:u64,
    nx1:u64, // 下个线程的PC
}

impl Default for TaskContext {
    fn default() -> Self {
        Self { x1: 0, x2: 0, x8: 0, x9: 0, x18: 0, x19: 0, x20: 0, x21: 0, x22: 0, x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, nx1: 0 }
    }
}

#[derive(Debug,PartialEq,Clone)]
/// 线程状态
enum State {
    Available, // 初始态：线程空闲，可被分配一个任务去执行
    Running,   // 运行态：线程正在执行
    Ready,     // 就绪态：线程已准备好，可恢复执行
}

/// 线程
struct Task {
    id: usize,            // 线程ID
    stack: Vec<u8>,       // 栈
    ctx: TaskContext,     // 当前指令指针(PC)和通用寄存器集合
    state: State,         // 执行状态
}

impl Task {
    /// 线程初始化
    fn new(id: usize) -> Self {
        Task {
            id,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: TaskContext::default(),
            state: State::Available,
        }
    }
}

pub struct Runtime{
    tasks:Vec<Task>,
    current:usize,
}

impl Runtime {
    pub fn new() -> Self {
        // 基本任务：主线程
        let base_task = Task {
            id: 0,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: TaskContext::default(),
            state: State::Running,
        };

        // 把主线程加入，然后加入其他空白线程
        let mut tasks = vec![base_task];
        let mut available_tasks: Vec<Task> = (1..MAX_TASKS).map(|i| Task::new(i)).collect();
        tasks.append(&mut available_tasks);
        Runtime {
            tasks,
            current: 0,
        }
    }

    pub fn init(&self) {
        unsafe {
            let r_ptr: *const Runtime = self;
            RUNTIME = r_ptr as usize;
        }
    }
    // 创建线程
    pub fn spawn(&mut self, f: fn()) {
        // 从线程控制块中找一块
        let available = self
            .tasks
            .iter_mut()
            .find(|t| t.state == State::Available)
            .expect("no available task.");

        let size = available.stack.len();
        unsafe {
            // 栈反向生长，所以先获得栈顶
            let s_ptr = available.stack.as_mut_ptr().offset(size as isize);
            // 字节 对齐
            let s_ptr = (s_ptr as usize & !7) as *mut u8;

            // 
            available.ctx.x1 = guard as u64;  //ctx.x1  is old return address
            available.ctx.nx1 = f as u64;     //ctx.nx1 is new return address
            available.ctx.x2 = s_ptr.offset(-32) as u64; //cxt.x2 is sp
        }
        available.state = State::Ready;
    }

    fn guard() {
        unsafe {
            let rt_ptr = RUNTIME as *mut Runtime;
            (*rt_ptr).t_return();
        };
    }

    fn t_return(&self) ->usize{
        todo!()
    }
}

