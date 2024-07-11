#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
// 正常输出
#[no_mangle]
fn main() -> i32 {
    println!("Hello, world1!");
    0
}
