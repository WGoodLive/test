pub mod context;

use crate::batch::run_next_app;
use core::arch::global_asm;
use riscv::register::{
    scause::{self,Exception,Trap}, stval, stvec, utvec::TrapMode
};
pub use context::TrapContext;

use crate::syscall::syscall;

global_asm!(include_str!("trap.S"));


pub fn init(){
    extern "C" {fn __alltraps();}
    unsafe{
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler(cx:&mut TrapContext) -> &mut TrapContext{
    let scause = scause::read(); // 中断原因
    let stval = stval::read();  // trap附加信息
    match scause.cause(){
        Trap::Exception(Exception::UserEnvCall)=>{
            cx.sepc +=4;// 转下一个指令
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize; // ？？？
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault)=>{
            println!("[kernel] PageFault in application, kernel killed it.");
            run_next_app();
        }
        // 如果打开了2_5priv_inst.rs,但是这个异常操作系统不处理(下面代码注释掉)，就是直接panic，结束shutdown
        Trap::Exception(Exception::IllegalInstruction)=>{
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            run_next_app();
        }

        _ =>{
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            ); // 任何代码一旦panic,不可恢复
        }
    }
    cx
}