#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
use riscv::register::sstatus::{self,SPP};
//用户模式尝试访问S模式的CSR寄存器
#[no_mangle]
fn main() -> i32 {
    println!("Try to access privileged CSR in U Mode");
    println!("Kernel should kill this application!");
    unsafe {
        sstatus::set_spp(SPP::User);
    }
    0
}