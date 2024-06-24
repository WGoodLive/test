use core::{arch::asm, ptr};
use log::*;

pub unsafe fn print_stack_trace()->(){
    unsafe{
        let mut a : *const usize;
        asm!("mv {},fp",out(reg) a);
        error!("fp:0x{:016x}",*a);
    }

    unsafe{
        let mut fp:*const usize;
        asm!("mv {},fp",out(reg) fp);
        println!("====Begin stack trace====");
        while fp != (0x1)as *const usize { // sp栈顶
            let saved_ra = *fp.sub(1);//ra 
            let saved_fp = *fp.sub(2);//调用者的fp
            println!("0x{:016x}, fp = 0x{:016x}", saved_ra, saved_fp);
            fp = saved_fp as *const usize;
        }
        println!("== End stack trace ==");
    }
}