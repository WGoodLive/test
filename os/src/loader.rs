use crate::config::*;
use crate::trap::context::TrapContext;
use core::arch::asm;


static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [
    KernelStack {data: [0; KERNEL_STACK_SIZE],}; MAX_APP_NUM
];

static USER_STACK: [UserStack; MAX_APP_NUM] = [
    UserStack {data: [0; USER_STACK_SIZE],}; MAX_APP_NUM
]; 
/// trap使用的内核栈
#[repr(align(4096))] // 设置对齐方式为4096字节
#[derive(Copy, Clone)]
struct KernelStack{
    data:[u8;KERNEL_STACK_SIZE],
}

impl KernelStack {
    /// 获取内核栈的栈顶
    /// 由于栈是向下生长的，所以栈顶就是栈底加上栈的大小
    fn get_sp(&self)->usize{
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    fn push_context(&self,cx:TrapContext) -> usize{
        //core::mem::size_of<T>() -> usize:获得结构体字节
       let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
       unsafe {
            *trap_cx_ptr = cx;
       }
       trap_cx_ptr as usize
    }
}

/// trap使用的用户栈
#[repr(align(4096))]
#[derive(Copy, Clone)]
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


/// Load nth user app at
/// [APP_BASE_ADDRESS + n * APP_SIZE_LIMIT, APP_BASE_ADDRESS + (n+1) * APP_SIZE_LIMIT).
pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    // load apps
    for i in 0..num_app {
        let base_i = get_base_i(i);
        // clear region
        // 法1
        // core::slice::from_raw_parts_mut(base_i as *mut u8, APP_SIZE_LIMIT).fill(0);
        // 法2
        (base_i..base_i + APP_SIZE_LIMIT)
            .for_each(|addr| unsafe { (addr as *mut u8).write_volatile(0) });
        // load app from data section to memory
        let src = unsafe {
            core::slice::from_raw_parts(app_start[i] as *const u8, app_start[i + 1] - app_start[i])
        };
        println!("{:#x}",base_i);
        let dst = unsafe { core::slice::from_raw_parts_mut(base_i as *mut u8, src.len()) };
        dst.copy_from_slice(src);
    }
    // Memory fence about fetching the instruction memory
    // It is guaranteed that a subsequent instruction fetch must
    // observes all previous writes to the instruction memory.
    // Therefore, fence.i must be executed after we have loaded
    // the code of the next app into the instruction memory.
    // See also: riscv non-priv spec chapter 3, 'Zifencei' extension.
    unsafe {
        asm!("fence.i");
    }
}
/// 返回app的个数
pub fn get_num_app() -> usize{ 
    extern "C"{
        fn _num_app();
    }
    unsafe{
        (_num_app as usize as *const usize).read_volatile()
    }
}

fn get_base_i(app_id:usize)->usize{
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

pub fn init_app_cx(app_id:usize)->usize{
    KERNEL_STACK[app_id].push_context(
        TrapContext::app_init_context(
            get_base_i(app_id),
            USER_STACK[app_id].get_sp()
        )
    )
}

