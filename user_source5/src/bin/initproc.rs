#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use user_lib::{
    fork,
    wait,
    exec,
    yield_,
};

#[no_mangle]
fn main()->isize{
    if(fork()==0){ 
        exec("user_shell\0");
    }else{
        loop{ // 根不能被回收，所以要执行到最后
            // 用户初始程序 initproc 对于资源的回收并不算及时，但是对于已经退出的僵尸进程，用户初始程序 initproc 最终总能够成功回收它们的资源
            let mut exit_code:i32 =0;
            let pid = wait(&mut exit_code); // 等待一个进程返回，退出码：标志每个程序的执行状态
            if pid == -1 { // 等待的进程不存在
                yield_();
                continue;
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid,
                exit_code,
            );
        }
    }
    0
}