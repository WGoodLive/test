use crate::config::*;
use crate::trap::context::TrapContext;
use core::arch::asm;


pub static KERNEL_STACK: KernelStack = KernelStack { data: [0; KERNEL_STACK_SIZE] };
pub static USER_STACK: UserStack = UserStack { data: [0; USER_STACK_SIZE] };
/// trap使用的内核栈
#[repr(align(4096))] // 设置对齐方式为4096字节
pub struct KernelStack{
    data:[u8;KERNEL_STACK_SIZE],
}

impl KernelStack {
    /// 获取内核栈的栈顶
    /// 由于栈是向下生长的，所以栈顶就是栈底加上栈的大小
    pub fn get_sp(&self)->usize{
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    pub fn push_context(&self,cx:TrapContext) -> &'static mut TrapContext{
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
pub struct UserStack{
    data:[u8;USER_STACK_SIZE],
}

impl UserStack {
    /// 获取用户栈的栈顶
    /// 由于栈是向下生长的，所以栈顶就是栈底加上栈的大小
    pub fn get_sp(&self)->usize{
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

pub fn loader(){
    extern "C"{
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe{
        core::slice::from_raw_parts(num_app_ptr.add(1),num_app+1)
    };
    // load apps
    for i in 0..num_app{
        unsafe {
            let base_i = get_base_i(i);
            let app_src = core::slice::from_raw_parts(
                app_start[i] as *const u8,
                app_start[i+1] - app_start[i]
            );

            // 法1
            core::slice::from_raw_parts_mut(base_i as *mut u8, APP_SIZE_LIMIT).fill(0);
            // 法2
            // (base_i..base_i+APP_SIZE_LIMIT).for_each(|addr|{
            //     unsafe {
            //         (addr as *mut u8).write_volatile(0)
            //     }
            // });
            let app_dst = core::slice::from_raw_parts_mut(
                base_i as *mut u8, 
                app_src.len()
            );
            app_dst.copy_from_slice(app_src);
        }        
        unsafe {
            asm!("fence.i");
        }
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

pub fn get_base_i(app_id:usize)->usize{
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}


