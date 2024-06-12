// 内核与 RustSBI 通信的相关功能实现在子模块 sbi 中


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