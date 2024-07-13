pub mod context;

use crate::timer::set_next_trigger;
use core::{arch::global_asm, f32::INFINITY, task};
use riscv::register::{
    scause::{self,Exception,Trap}, stval, stvec, utvec::TrapMode
};
use riscv::register::scause::Interrupt;
pub use context::TrapContext;

use crate::{syscall::syscall, task::{exit_current_and_run_next, TASK_MANAGER}};

global_asm!(include_str!("trap.S"));


pub fn init(){
    println!("trap start...");
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
            panic!("[kernel] Cannot continue!");
            // exit_current_and_run_next();
        }
        // 如果打开了2_5priv_inst.rs,但是这个异常操作系统不处理(下面代码注释掉)，就是直接panic，结束shutdown
        Trap::Exception(Exception::IllegalInstruction)=>{
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            panic!("[kernel] Cannot continue!");
            // exit_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer)=>{
            set_next_trigger();
            crate::task::suspend_current_and_run_next();
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

use riscv::register::sie;

pub fn enable_timer_interrupt(){
    unsafe {
        sie::set_stimer();
    }
}