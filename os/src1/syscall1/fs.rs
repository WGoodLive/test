use alloc::task;

use crate::fs::inode::open_file;
use crate::fs::OpenFlags;
use crate::mm::{translated_str, UserBuffer};
use crate::task::processor::{current_task, current_user_token};
use crate::{mm::translated_byte_buffer};

const FD_STDIN: usize = 0;
/// 因为sys_write写入的变化，所以原来的sys_read的console_getchar已经不需要了
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
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

const FD_STDOUT:usize = 1;

pub fn sys_write(fd:usize,buf:*const u8,len:usize) -> isize{
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
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
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(
        path.as_str(),
        OpenFlags::from_bits(flags).unwrap()
    ){
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode); 
        fd as isize
    }else{
        return -1;
    }
}

/// 关闭成功0,关闭失败-1（文件不存在）
pub fn sys_close(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() { // 文件描述符越界了
        return -1;
    }
    if inner.fd_table[fd].is_none() { // 没有文件
        return -1;
    }
    inner.fd_table[fd].take(); // 将这个Arc变成None，就说明这个文件描述符失效了，文件不存在
    0
}