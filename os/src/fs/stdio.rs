use super::File;
use crate::fs::UserBuffer;
use crate::sbi::console_getchar;
use crate::task::suspend_current_and_run_next;

/// 是的，标准输入输出流（Standard Input/Output Streams）
/// 是一种用于在程序和外部设备（如键盘、显示器等）之间进行数据交换的机制。

/// 标准输入（外设交换信息）
pub struct Stdin;
/// 标准输出（外设交换信息）
pub struct Stdout;

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        false
    }
    /// 把外设输入写到用户空间传的参数的地址处
    fn read(&self, mut user_buf: UserBuffer) -> usize {
        assert_eq!(user_buf.len(), 1);
        
        let mut c: usize;
        loop {
            // 获取键盘输入
            c = console_getchar();
            if c == 0 {
                suspend_current_and_run_next();
                continue;
            } else {
                break;
            }
        }
        let ch = c as u8;
        unsafe {
            user_buf.buffers[0].as_mut_ptr().write_volatile(ch);
        }
        1
    }
    fn write(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot write to stdin!");
    }
}

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }
    fn writable(&self) -> bool {
        true
    }
    fn read(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot read from stdout!");
    }
    fn write(&self, user_buf: UserBuffer) -> usize {
        for buffer in user_buf.buffers.iter() {
            print!("{}", core::str::from_utf8(*buffer).unwrap());
        }
        user_buf.len()
    }
}