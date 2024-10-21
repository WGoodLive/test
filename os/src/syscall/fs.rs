use crate::task::processor::current_user_token;
use crate::task::suspend_current_and_run_next;
use crate::{mm::page_table::translated_byte_buffer};
use crate::sbi::console_getchar;

const FD_STDIN: usize = 0;
/// buf：应用接收字符的地址，然后字符写入这里
pub fn sys_read(fd:usize,buf:*const u8,len:usize) -> isize{
    match fd{
        FD_STDIN=>{
            assert_eq!(len, 1, "Only support len = 1 in sys_read!");
            let mut c:usize;
            loop{
                c = console_getchar();
                if c==0{
                    suspend_current_and_run_next();
                    continue;
                }else {
                    break;
                }
            }
            let ch = c as u8;
            let mut buffer = translated_byte_buffer(current_user_token(), buf, len);
            unsafe {
                buffer[0].as_mut_ptr().write_volatile(ch);
            }
            1
        }
        _ => {
            panic!("Unsupposed fn in sys_read!")
        }
    }
}

const FD_STDOUT:usize = 1;

pub fn sys_write(fd:usize,buf:*const u8,len:usize) -> isize{
    match fd { 
        FD_STDOUT =>{
            let buffers = translated_byte_buffer(current_user_token(), buf, len);
            for buffer in buffers {
                print!("{}", core::str::from_utf8(buffer).unwrap());
            }
            len as isize
        },
        _ => {
            panic!("sys_write not support this fd...");
        }
    }
}

