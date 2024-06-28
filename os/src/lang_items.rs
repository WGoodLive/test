// os/src/lang_items.rs
use core::panic::PanicInfo;
// 编译指导属性，用于标记核心库core中的 panic! 宏要对接的函数
use crate::sbi::shutdown;
/*
panic! 宏最典型的应用场景包括
    断言宏 assert! 失败或者对 Option::None/Result::Err 进行 unwrap 操作。
所以Rust编译器在编译程序时，从安全性考虑，需要有 panic! 宏的具体实现。
*/
#[panic_handler]  // 发生错误时候，调用下面的函数
fn panic(_info:&PanicInfo) -> !{ // core包里处理异常的

    if let Some(location) = _info.location(){

        println!("\nFile:{}:{},cause:{}",
        location.file(),location.line(),_info.message().unwrap()
        );
        crate::stack_trace::print_stack_trace();
    }else{
        println!("Panicked:{}",_info.message().unwrap());
    }
    shutdown(true)
}