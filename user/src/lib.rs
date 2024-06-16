#![no_std]
#![feature(linkage)] // 允许弱链接
#![feature(panic_info_message)]

// 项目文件
#[macro_use]
pub mod console;
mod lang_items;
mod syscall;


// 我的理解：
// make build的时候，顺便把第一个软件bin/xx.rs加载到了指定位置，
// 然后,bin/xx.rs的main作为作为强链接，就没lib.rs的main啥事了
#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> !{
    clear_bss();
    println!("Hello, world!");
    exit(main());
    panic!("unreachable after sys_exit!");
}

/// 弱引用，由于bin和lib里面都有，main，让这里成为备选项目，这样尽管bin里没main,也能编译通过，不过运行出错
#[linkage="weak"] 
#[no_mangle]
fn main() -> i32{
    panic!("Cannot find main!");
}

/// .bss段清0
fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }
    (start_bss as usize..end_bss as usize).for_each(|addr| unsafe {
        (addr as *mut u8).write_volatile(0);
    });
}

// 导包
use syscall::*;
pub fn write(fd:usize,buf:&[u8]) ->isize{
    sys_write(fd, buf)
}

pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}