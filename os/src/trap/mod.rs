pub mod context;

use crate::timer::set_next_trigger;
use core::{arch::global_asm, f32::INFINITY, task};
use riscv::register::{
    mstatus::SPP, scause::{self,Exception,Trap}, sstatus, stval, stvec, utvec::TrapMode
};

use riscv::register::scause::Interrupt;
pub use context::TrapContext;

use crate::{syscall::syscall, task::{exit_current_and_run_next, TASK_MANAGER}};

global_asm!(include_str!("trap.S"));


pub fn init(){
    println!("trap start...");
    extern "C" {fn __alltraps();}
    unsafe{
        
    sstatus::set_sie()  // 打开内核态中断
    // sstatus::clear_sie() // 关闭内核态中断
    }
    unsafe{
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_handler(cx:&mut TrapContext)->&mut TrapContext{
    match sstatus::read().spp() {
        sstatus::SPP::Supervisor=>{
            println!("kernel interrept...");
            trap_kernel_handler(cx)
        },
        sstatus::SPP::User=>trap_user_handler(cx),
    }
}

/// handle an interrupt, exception, or system call from user space
pub fn trap_user_handler(cx:&mut TrapContext) -> &mut TrapContext{
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

static mut KERNEL_INTERRUPT_TRIGGERED: bool = false;

/// 检查内核中断是否触发
pub fn check_kernel_interrupt() -> bool {
    unsafe { (&mut KERNEL_INTERRUPT_TRIGGERED as *mut bool).read_volatile() }
}

/// 标记内核中断已触发
pub fn trigger_kernel_interrupt() {
    unsafe {
        (&mut KERNEL_INTERRUPT_TRIGGERED as *mut bool).write_volatile(true);
    }
}
pub fn trap_kernel_handler(cx:&mut TrapContext) ->&mut TrapContext{
    let scause = scause::read();
    let stval = stval::read();

    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer)=>{
            println!("supervisorTimer is coming...");
            trigger_kernel_interrupt(); // 标记内核中断可以抢占，调试代码，可以删除
            set_next_trigger(); // 说明时间片到了，才会触发这个
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            panic!("[kernel] PageFault in kernel, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.", stval, cx.sepc);
        }
        _ => {
            // 其他的内核异常/中断
            panic!("unknown kernel exception or interrupt");
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