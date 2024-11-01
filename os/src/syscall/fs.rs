use alloc::sync::Arc;
use alloc::task;

use crate::fs::inode::open_file;
use crate::fs::pipe::make_pipe;
use crate::fs::{pipe, OpenFlags};
use crate::mm::{translated_refmut, translated_str, UserBuffer};
use crate::task::processor::{current_task, current_user_token};
use crate::{mm::translated_byte_buffer};

use super::current_process;

/// 因为sys_write写入的变化，所以原来的sys_read的console_getchar已经不需要了
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        drop(inner);
        file.read(
            UserBuffer::new(translated_byte_buffer(token, buf, len))
        ) as isize
    } else {
        -1
    }
}


pub fn sys_write(fd:usize,buf:*const u8,len:usize) -> isize{
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len(){
        return -1;
    }
    if let  Some(file) = &inner.fd_table[fd]{
        let file = file.clone();// 找到文件，比如OSinode
        drop(inner); 
        file.write( // 向比如OSInode的Inode里面写入数据，这样OSInode访问Inode就相当于被写入了
            UserBuffer::new(translated_byte_buffer(token, buf, len))
        ) as isize
    }else {
        -1
    }
}

/// 功能：打开一个常规文件，并返回可以访问它的文件描述符。
/// 参数：path 描述要打开的文件的文件名（简单起见，文件系统不需要支持目录，所有的文件都放在根目录 / 下），
/// flags 描述打开文件的标志，具体含义下面给出。
/// 返回值：如果出现了错误则返回 -1，否则返回打开常规文件的文件描述符。可能的错误原因是：文件不存在。
/// syscall ID：56
pub fn sys_open(path: *const u8, flags: u32) -> isize{
    let token = current_user_token();
    let process = current_process();

    let path = translated_str(token, path);
    if let Some(inode) = open_file(
        path.as_str(),
        OpenFlags::from_bits(flags).unwrap()
    ){
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode); 
        fd as isize
    }else{
        return -1;
    }
}

/// 关闭成功0,关闭失败-1（文件不存在）
pub fn sys_close(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() { // 文件描述符越界了
        return -1;
    }
    if inner.fd_table[fd].is_none() { // 没有文件
        return -1;
    }
    inner.fd_table[fd].take(); // 将这个Arc变成None，就说明这个文件描述符失效了，文件不存在
    0
}

pub fn sys_pipe(pipe:*mut usize)->isize{
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read,pipe_write) = make_pipe();

    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);

    // pipe[0] = read_fd
    *translated_refmut(token, pipe) = read_fd;
    // pipe[1] = write_fd
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}

/// 功能：将进程中一个已经打开的文件复制一份并分配到一个新的文件描述符中。
/// 参数：fd 表示进程中一个已经打开的文件的文件描述符。
/// 返回值：如果出现了错误则返回 -1，否则能够访问已打开文件的新文件描述符。
/// 可能的错误原因是：传入的 fd 并不对应一个合法的已打开文件。
/// syscall ID：24
pub fn sys_dup(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}