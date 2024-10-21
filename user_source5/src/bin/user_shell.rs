#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8; // 换行
const CR: u8 = 0x0du8; // 回车(光标移动到行头)
const DL: u8 = 0x7fu8; // delete
const BS: u8 = 0x08u8; // 退格

use alloc::string::String;
use user_lib::{fork, exec, waitpid, yield_};
use user_lib::console::getchar;

#[no_mangle]
pub fn main() -> i32{
    println!("Rust user shell start to do...");
    let mut line:String = String::new();
    print!("[user]>> ");
    loop{
        let c = getchar();
        match c {
            LF | LR =>{
                print("");
                if !line.is_empty(){
                    line.push('\0');
                    let pid = fork();
                    if pid == 0{
                        if exec(line.as_str()) == -1{  // 找不到可执行文件
                            println!("Error when executing!");
                            return -4;
                        }
                        unreachable!(); // ????
                    }else{ // 父进程等待子进程退出，否则不能再申请新的子进程
                        let mut exit_code:i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        println!(
                            "Shell: Process {} exited with code {}",
                            pid, exit_code
                        );
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char); // 光标位置前移
                    print!(" ");  // 覆盖旧字符
                    print!("{}", BS as char); // 光标前移
                    line.pop();
                }
            }
            _ =>{
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}