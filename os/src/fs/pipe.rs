use alloc::sync::{Arc, Weak};

use crate::{mm::UserBuffer, sync::UPSafeCell, task::suspend_current_and_run_next};

use super::File;


/// 管道端  
/// 不允许向读端写入，也不允许从写端读取
pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<UPSafeCell<PipeRingBuffer>>,
}

impl Pipe {
    // 为管道设置读端
    pub fn read_end_with_buffer(buffer:Arc<UPSafeCell<PipeRingBuffer>>)->Self{
        Self { readable: true, writable: false, buffer}
    }
    // 为管道设置写端
    pub fn write_end_with_buffer(buffer:Arc<UPSafeCell<PipeRingBuffer>>)->Self{
        Self { readable: false, writable: true, buffer}
    }

}

impl File for Pipe {
    fn read(&self, buf: UserBuffer) -> usize {
        assert!(self.readable());
        let want_to_read = buf.len();
        // 字节迭代器
        let mut buf_iter =  buf.into_iter();
        let mut already_read = 0usize;
        loop{
            let mut ring_buffer = self.buffer.exclusive_access();
            // 可以读的数
            let loop_read = ring_buffer.available_read();
            // 目前没有可以读的
            // 要么写端关闭，要么先放弃cpu
            if loop_read==0{
                if ring_buffer.all_write_ends_closed(){
                    return already_read;
                }
                drop(ring_buffer);
                // 等一波进程运行完
                suspend_current_and_run_next();
                continue;
            }
            // 有读的
            // 按需读取：全读 / 读不完
            for _ in 0..loop_read{
                if let Some(byte_ref) = buf_iter.next(){
                    unsafe{
                        *byte_ref = ring_buffer.read_byte();
                    }
                    already_read+=1;
                    if already_read == want_to_read{
                        return want_to_read;
                    }
                }else{
                    return already_read;
                }
            }
        }
    }

    fn write(&self, buf: UserBuffer) -> usize {
        assert!(self.writable());
        let want_to_write = buf.len();
        let mut buf_iter = buf.into_iter();
        let mut already_write = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access();
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            // write at most loop_write bytes
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    already_write += 1;
                    if already_write == want_to_write {
                        return want_to_write;
                    }
                } else {
                    return already_write;
                }
            }
        }
    }

    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }
}

const RING_BUFFER_SIZE: usize = 32;

/// 缓冲区状态
#[derive(Copy, Clone, PartialEq)]
enum RingBufferStatus {
    FULL,
    EMPTY,
    NORMAL,
}

/// 而管道自身，也就是那个带有一定大小缓冲区的字节队列  
/// 每个读端或写端中都保存着所属管道自身的强引用计数，且我们确保这些引用计数只会出现在管道端口 `Pipe` 结构体中。
pub struct PipeRingBuffer {
    // 三个字段维护一个循环队列
    arr: [u8; RING_BUFFER_SIZE],// 存放数据
    head: usize, // 循环队列队头的下标
    tail: usize, // 循环队列队尾的下标

    status: RingBufferStatus,
    // 它的写端的一个弱引用计数，这是由于在某些情况下需要确认该管道所有的写端是否都已经被关闭了，
    // 如果写端全部关闭，通过这个字段返回None
    write_end: Option<Weak<Pipe>>, 
}

impl PipeRingBuffer {
    // 新建(不是初始化)
    pub fn new() -> Self{
        Self{
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: RingBufferStatus::EMPTY,
            write_end: None,
        }
    }

    // 建立写端的弱引用
    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }

    /// 读一个字节
    pub fn read_byte(&mut self)-> u8{
        self.status = RingBufferStatus::NORMAL;
        let c = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail { // 也可以用和，但是和约束条件高，但是麻烦
            self.status = RingBufferStatus::EMPTY;
        }
        c
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::NORMAL;
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::FULL;
        }
    }

    /// 可以读的字节
    pub fn available_read(&self) -> usize{
        if self.status == RingBufferStatus::EMPTY {
            0
        } else {
            if self.tail > self.head {
                self.tail - self.head
            } else {
                self.tail + RING_BUFFER_SIZE - self.head
            }
        }
    }

    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::FULL {
            0
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }

    /// 判断写端是否全部关闭
    pub fn all_write_ends_closed(&self)->bool{
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }

}

pub fn make_pipe() -> (Arc<Pipe>,Arc<Pipe>){
    unsafe {
        let buffer = Arc::new(
            UPSafeCell::new(PipeRingBuffer::new())
        );
        // 这里如果不clone，Arc<buffer>会转移所有权的
        let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
        let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
        buffer.exclusive_access().set_write_end(&write_end);
        (read_end,write_end)
    }
}