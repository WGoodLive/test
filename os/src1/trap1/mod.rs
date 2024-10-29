pub mod context;

use crate::{ task::processor::{current_trap_cx, current_user_token}, timer::set_next_trigger, TRAMPOLINE, TRAP_CONTEXT};
use core::{arch::{asm, global_asm}};
use riscv::register::{
    scause::{self,Exception,Trap}, stval, stvec, utvec::TrapMode
};
use riscv::register::scause::Interrupt;
pub use context::TrapContext;

use crate::{syscall::syscall, task::{exit_current_and_run_next}};

global_asm!(include_str!("trap.S"));


fn set_kernel_trap_entry(){
    unsafe{
        stvec::write(trap_from_kernel as usize, TrapMode::Direct)
    }
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_return() -> ! {
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }

    // 得到汇编的__restore的偏移量，偏移
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;

    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}", // 这里用偏移地址的原因：内核与用户的虚拟地址不一样
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
    panic!("Unreachable in back_to_user!");
}

#[no_mangle]
pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}

pub fn init(){
    println!("trap start...");
    extern "C" {fn __alltraps();}
    unsafe{
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let scause = scause::read(); // 中断原因
    let stval = stval::read();  // trap附加信息
    match scause.cause(){
        Trap::Exception(Exception::UserEnvCall)=>{
            let mut cx = current_trap_cx();
            cx.sepc +=4;// 转下一个指令
            // new_task里面的x[10]是0
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]); // ？？？
            
            // cx在exec的时候可能被修改
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) | Trap::Exception(Exception::LoadPageFault)=>{
            println!("[kernel] PageFault in application, kernel killed it.");
            exit_current_and_run_next(-2);
        }
        // 如果打开了2_5priv_inst.rs,但是这个异常操作系统不处理(下面代码注释掉)，就是直接panic，结束shutdown
        Trap::Exception(Exception::IllegalInstruction)=>{
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next(-3);
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
    trap_return();
}

use riscv::register::sie;

pub fn enable_timer_interrupt(){
    unsafe {
        sie::set_stimer();
    }
}