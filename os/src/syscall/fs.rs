use crate::{mm::page_table::translated_byte_buffer, task::current_user_token};

const FD_STDOUT:usize = 1;

pub fn sys_write(fd:usize,len:usize,buf:*const u8,) -> isize{
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

