//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`syscall`]: System call handling and implementation
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`batch::run_next_app()`] and for the first time go to
//! userspace.
// #![warn(missing_docs)]
// #![deny(warnings)] // 把警告处理成错误

#![no_std] // 不用标准库，用核心库core
#![no_main]
// start 语义项代表了标准库 std 在执行应用程序之前需要进行的一些初始化工作。
// 由于我c们禁用了标准库，编译器也就找不到这项功能的实现了。 
// 通过禁止main函数，就没有了所谓的初始化操作
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

// 定义了一个模块board，并且指定了该模块的路径为boards/qemu.rs
#[path = "boards/qemu.rs"]
mod board;
extern crate alloc;
pub mod mm;

use core::arch::{global_asm,asm};

use log::{debug, error, info, trace, warn};
use logging::init_Log;


pub use config::*;
// 使用宏
#[macro_use]
mod console;
mod sync;
mod logging;
mod lang_items;
mod timer;
mod drivers;
mod sbi;// 用户最小化环境构建
pub mod syscall;
pub mod trap;
pub mod config;
pub mod task;
// pub mod batch; // 应用加载 + 执行切换

mod fs;
#[macro_use]
extern crate bitflags;

global_asm!(include_str!("entry.asm"));
// 把entry.asm变成字符串通过global_asm嵌入到代码中
global_asm!(include_str!("link_app.S"));
// 一开始这个文件并没有，但是通过rustc的参数-Zbuild-std=core,std，编译器会自动生成link_app.S文件
// 因为注释的问题，


#[no_mangle]
fn pre_section(){
    extern "C"{
        fn etext();
        fn stext();
        fn erodata();
        fn srodata();
        fn edata();
        fn sdata();
        fn ebss();
        fn sbss();
    }
    info!(".text [{:#x},{:#x})",stext as usize,etext as usize);
    info!(".rodata [{:#x},{:#x})",srodata as usize,erodata as usize);
    info!(".data [{:#x},{:#x})",sdata as usize,edata as usize);
    info!(".bss [{:#x},{:#x})",sbss as usize,ebss as usize);
}



#[no_mangle]
fn clear_bss(){
    extern "C"{
        fn sbss();
        fn ebss();
    }

    (sbss as usize..ebss as usize).for_each(|a|{
        unsafe{(a as *mut u8 ).write_volatile(0)} // 字节置0
    });
}
// 尽管 ! 类型表示函数不会返回，但它并不等同于 void（在 C 或 C++ 中的概念）。在 Rust 中，void 类型是不存在的；所有函数都必须有一个返回类型，即使这个类型是 !。此外，! 类型也不能用作其他类型的子类型（即它不是一个“bottom type”），这意味着你不能将一个返回 ! 的函数赋值给一个期望返回其他类型的变量。
/* 
#[no_mangle] // 不是 #![no_mangle] 
// 众所周知，C程序的入口并不是main函数，而是_start函数，只不过gcc链接器默认会依赖一些库
extern "C" fn _start(){ // 必须是 _start
    loop {
        
    }; 
}
*/


//1. 在 rust_main 函数的开场白中，我们将第一次在栈上分配栈帧并保存函数调用上下文，它也是内核运行全程中最底层的栈帧。
//1.1 在内核初始化中，需要先完成对 .bss 段的清零
//1.2 我们就在 rust_main 的开头完成这一工作，由于控制权已经被转交给 Rust 
//2. 没有返回值的函数。rust没return的函数默认返回`()` ，不是!类型
#[no_mangle] //防止编译器更改这里定义的名字
pub fn rust_main() -> ! {
    clear_bss();
    println!("[kernel] Hello, world!");
    mm::init();
    mm::remap_test();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    fs::list_apps();
    task::add_initproc();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}
// pub fn rust_main() -> !{ 

//     clear_bss();    // 给栈初始化
//     pre_section();
//     init_Log();
//     println!("main start...");
//     trap::init();
//     {
//         mm::init();
//         mm::memory_set_new::remap_test();
//         panic!("end...");
//     }
//     loader::load_apps();
//     timer::init_timer();
//     task::run_first_task();
//     // ----------------------------正常退出--------------------------
//     panic!("main error...");
// }



























































