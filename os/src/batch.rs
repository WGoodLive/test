
use crate::trap::context::TrapContext;
use lazy_static::*;
use core::{arch::asm};
use crate::{sbi::shutdown, sync::UPSafeCell};

// -------------------常量-------------------
const MAX_APP_NUM:usize = 6;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;
const USER_STACK_SIZE: usize = 4096 * 2;
const KERNEL_STACK_SIZE: usize = 4096 * 2;



//----------------全局变量-----------------------


// 儲存在.bss段
static KERNEL_STACK: KernelStack = KernelStack { data: [0; KERNEL_STACK_SIZE] };
static USER_STACK: UserStack = UserStack { data: [0; USER_STACK_SIZE] };
// lazy_static! 宏提供了全局变量的运行时初始化功能。
// 在使用全局变量之前，才自动初始化，并且不用人为初始化
// lazy_static! 宏会自动处理线程安全问题，无需手动加锁
// lazy_static!{全局变量 = {初始化工作};}
lazy_static! {
    // ref：保留值的所有权但同时又需要访问它时,就是引用他的意思 
    static ref APP_MANAGER:UPSafeCell<AppManager> = 
    unsafe{
        UPSafeCell::new({
            extern "C" {
                fn _num_app();
            }
            // 将_num_app转换为usize类型，并将其转换为指向usize类型的不可变指针
            let num_app_str = _num_app as usize as *const usize; 
            let num_app = num_app_str.read_volatile();
            // read_volatile方法是Rust标准库中的ptr模块提供的一个方法，
            // 它从self（一个可变引用）中读取值，而不会移动它。
            // 这与read方法不同，read方法会移动被引用的值。
            let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
            // num_app_str下移一个元素长度(略过.quad length长度)，然后读取num_app+1个元素
            let app_start_raw:&[usize] = core::slice::from_raw_parts(num_app_str.add(1), num_app + 1);

            app_start[..=num_app].copy_from_slice(app_start_raw);

            AppManager{
                num_app,
                current_app:0,
                app_start,
            }
        })
    };
}

// -------------------------结构体-------------------------

/// 全局变量：应用管理器  
/// 1.可以采用static mut声明全局变量，但是这是unsafe!  
/// 2.单独使用 static 而去掉 mut 的话，我们可以声明一个初始化之后就不可变的全局变量  
///     对此我们需要使用RefCell让AppManager可以被修改  
/// 问题出现：
/// 1. `static RefCell A = 10; * A=11;` 报错，因为线程不安全
struct AppManager{
    num_app: usize,
    current_app:usize,
    app_start:[usize;MAX_APP_NUM+1],
}

impl AppManager { 
    /// 输出app信息
    pub fn print_app_info(&self){
        println!("[Kernel] num_app = {}",self.num_app);
        for i in 0..self.num_app{
            println!("[Kernel] app_{} : [{:#x},{:#x}]",i,self.app_start[i],self.app_start[i+1]);
        }
    }

    /// 得到当前app编号
    pub fn get_current_app(&self)->usize{
        self.current_app
    }

    pub fn move_next_app(&mut self){
        self.current_app+=1;
    }

    unsafe fn load_app(&self,app_id:usize){
        if app_id>= self.num_app{
            println!("All applications completd!");
            // main -> load_app -> panic -> print_stack_trace ?????
            shutdown(false);
        }
        println!("[Kernel] load app_{}",app_id);

        // clear app area
        core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0); // [_;Len] -> [fill_value;Len] 
        // app_id从0开始编号
        let app_src = core::slice::from_raw_parts(
        self.app_start[app_id] as *const u8,
        self.app_start[app_id + 1] - self.app_start[app_id],// 对齐方式是字节，取址方式看对齐方式
        );
        let app_dst = core::slice::from_raw_parts_mut(
            APP_BASE_ADDRESS as *mut u8,
            app_src.len()
        );
        // 在这一点上也体现了冯诺依曼计算机的 代码即数据 的特征。
        app_dst.copy_from_slice(app_src); // 在0x80400000地址内存填充

        // memory fence about fetching the instruction memory
        asm!("fence.i");
    }
}

/// trap使用的内核栈
#[repr(align(4096))] // 设置对齐方式为4096字节
struct KernelStack{
    data:[u8;KERNEL_STACK_SIZE],
}

impl KernelStack {
    /// 获取内核栈的栈顶
    /// 由于栈是向下生长的，所以栈顶就是栈底加上栈的大小
    fn get_sp(&self)->usize{
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    fn push_context(&self,cx:TrapContext) -> &'static mut TrapContext{
        //core::mem::size_of<T>() -> usize:获得结构体字节
       let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
       unsafe{
        *cx_ptr = cx;
       }
       unsafe{
        cx_ptr.as_mut().unwrap()
       }
    }
}

/// trap使用的用户栈
#[repr(align(4096))]
struct UserStack{
    data:[u8;USER_STACK_SIZE],
}

impl UserStack {
    /// 获取用户栈的栈顶
    /// 由于栈是向下生长的，所以栈顶就是栈底加上栈的大小
    fn get_sp(&self)->usize{
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}


/// init batch subsystem
pub fn init() {
    print_app_info();
}

/// print apps info
pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

/// run next app
pub fn run_next_app()->!{
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.current_app;
    unsafe{
        app_manager.load_app(current_app);
    }
    app_manager.move_next_app();
    drop(app_manager); // 这是个临时指针，没有所有权
    // 手动释放资源

    extern "C"{
        fn __restore(cx_addr: usize); // 函数声明
    }

    unsafe{
        // 这个代码有点抽象在于，他没有保存寄存器
        // 因为是直接加载新程序，除了函数入口，栈帧，其他的东西统统不用保存
        __restore(KERNEL_STACK.push_context(
            TrapContext::app_init_context(
                APP_BASE_ADDRESS, 
                USER_STACK.get_sp(),  
            )
        ) as *const _ as usize); // 程序开始
    }

    panic!("unreachable in batch::currect_app!");
}

