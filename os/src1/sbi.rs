// 内核与 RustSBI 通信的相关功能实现在子模块 sbi 中



const SBI_SET_TIMER:usize = 0;


/// use sbi call to getchar from console (qemu uart handler)
#[allow(unused)]
pub fn console_getchar() -> usize {
    #[allow(deprecated)]
    sbi_rt::legacy::console_getchar()
}


pub fn console_putchar(c:usize){
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}

pub fn shutdown(failure:bool) -> !{
    use sbi_rt::{system_reset,NoReason,Shutdown,SystemFailure};

    if !failure{
        system_reset(Shutdown, NoReason);
    }else {
        system_reset(Shutdown, SystemFailure);
    }
    unreachable!()
}

/// set_timer 调用，是一个由 SEE 提供的标准 SBI 接口函数，它可以用来设置 mtimecmp 的值。
pub fn set_timer(timer:usize){
    sbi_rt::set_timer(timer as _);
}
