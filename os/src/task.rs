// 处理任务
use crate::config::*;
use crate::{sbi::shutdown, sync::UPSafeCell};
use crate::loader::*;
use lazy_static::*;
use crate::trap::TrapContext;
lazy_static!{
    static ref APP_NUM:UPSafeCell<usize> = {
        unsafe{UPSafeCell::new(0)}
    };
}

/// run next app
pub fn run_next_app()->!{
    let mut t = APP_NUM.exclusive_access(); 
    if(*t>=3){
        shutdown(false)
    }
    drop(t);
    extern "C"{
        fn __restore(cx_addr: usize); // 函数声明
    }

    unsafe{
        // 这个代码有点抽象在于，他没有保存寄存器
        // 因为是直接加载新程序，除了函数入口，栈帧，其他的东西统统不用保存
        __restore(KERNEL_STACK.push_context({
            let cx = TrapContext::app_init_context(
                get_base_i(*(APP_NUM.exclusive_access())), 
                USER_STACK.get_sp(),  
            );
            
            println!("{:#x}",get_base_i(*(APP_NUM.exclusive_access())));
            println!("{:#x}", USER_STACK.get_sp());
            *(APP_NUM.exclusive_access())+=1;
            cx
        }
        ) as *const _ as usize);
    }

    panic!("unreachable in batch::currect_app!");
}