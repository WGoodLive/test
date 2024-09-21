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

// 链接器里面链接地址可以随便设定，都可以qemu-riscv64
// 尽管这里改，都可以随便运行，但是为了自己的操作系统也可以运行这个，需要设置个绝对地址、
// 我们也许可以将二进制文件生成为位置无关的，但是内核在加载的时候根据其实际加载的位置可能需要对二进制文件中的某些符号进行动态重定位，这大概需要更加完善的二进制文件解析和修改功能，对于我们教学内核来说完全没有必要。目前这种直接拷贝的方法还是比较简单且恰当
#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> !{
    clear_bss();
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

//  yield 是 Rust 的关键字，因此我们只能将应用直接调用的接口命名为 yield_ 。
pub fn yield_()->isize{
    sys_yield()
}

pub fn get_time() -> isize {
    sys_get_time()
}
