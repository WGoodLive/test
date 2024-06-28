#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use core::arch::asm;
// 用户模式尝试使用S模式的特权指令
#[no_mangle]
fn main()->i32{
    println!("Try to run S-mode instruction in U mode");
    println!("Kernel will kill this application!");

    unsafe{
        asm!("sret"); // 会出错
    }
    0
    // panic!("ddd"); // 调用操作系统的panic，不会用自己的
}