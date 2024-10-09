

extern crate alloc;

use core::fmt::Debug;

use super::address::PhysPageNum;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use core::fmt::{self,  Formatter};
/// 定义物理页帧管理器需要提供的功能
trait FrameAllocator{
    fn new()->Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self,ppn:PhysPageNum);
}
/// 栈式页帧管理，具体的管理分配的物理页帧  
pub struct StackFrameAllocator{
    // 物理页号区间 [ current , end ) 此前均 从未 被分配出去过
    current:usize,
    end:usize,
    recycled:Vec<usize>,
    //而向量 recycled 以后入先出的方式保存了被回收的物理页号（注：我们已经自然的将内核堆用起来了）
}

impl FrameAllocator for StackFrameAllocator {
    // 创建实例
    fn new()->Self {
        Self { current: 0, end: 0, recycled: Vec::new() }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop(){
            Some(ppn.into())
        }else{
            if(self.current==self.end){
                None
            }
            else{
                self.current+=1;
                Some((self.current-1).into())
            }
        }
    }

    /// 在实现cow机制的时候，这个没实现对共享引用的判断和处理
    fn dealloc(&mut self,ppn:PhysPageNum) {

        
        let ppn = ppn.0;
        // 看看是否已经被分配(已回收/未分配的情况都是不能销毁的)
        if ppn>self.current || self.recycled.iter().find(|&v|*v == ppn).is_some(){
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // 不知道后面这些页面之间，脏页会不会与干净页有不同
        // if(ppn == self.current-1){
        //     self.current-=1;
        //     return;
        // }

        self.recycled.push(ppn);
    }
}

impl StackFrameAllocator{
    pub fn init(&mut self,l:PhysPageNum,r:PhysPageNum){
        self.current = l.0;
        self.end = r.0;
    }
}

use crate::{mm::{address::PhysAddr, MEMORY_END}, println, sync::UPSafeCell};
type FrameAllocatorImpl = StackFrameAllocator;

lazy_static!{
    pub static ref FRAME_ALLOCATOR:UPSafeCell<FrameAllocatorImpl> = unsafe {
        UPSafeCell::new(FrameAllocatorImpl::new())
    };
}



/// 对物理页帧的封装
pub struct FrameTracker{
    pub ppn:PhysPageNum,
}
impl FrameTracker {
    pub fn new(ppn:PhysPageNum) ->Self{
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array{
            *i=0;
        }
        Self { ppn }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) { // 正好被人无法访问这页，正好回收他
        frame_dealloc(self.ppn);
    }
}

impl Debug for FrameTracker{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // write!(f,"FrameTracker:{}",&self.ppn.0)
        // f.debug_struct("FrameTracker")
        // .field("ppn:", &self.ppn.0)
        // .finish()

        // write!(f,"FrameTracker:PPN={:#x}",self.ppn.0);

        // 不拿所有权，会复制
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

// -------------给其他内核模块提供的接口---------------------------- //
pub fn frame_alloc() -> Option<FrameTracker>{  
    FRAME_ALLOCATOR.exclusive_access()
    .alloc().map(|ppn| FrameTracker::new(ppn))
}

fn frame_dealloc(ppn:PhysPageNum){
    FRAME_ALLOCATOR.exclusive_access()
    .dealloc(ppn);
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();

        println!("{:?}", frame);
        v.push(frame);
        println!("5len:{}",v.len());
    }
    v.clear();
    println!("0len:{}",v.len());
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
        println!("5len:{}",v.len());
    }
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    println!("10len:{}",v.len());
    drop(v);
    println!("frame_allocator_test passed!");
}

pub fn init_frame_allocator(){
    extern "C"{
        fn ekernel();
    }

    FRAME_ALLOCATOR.exclusive_access()
    .init(PhysAddr::from(ekernel as usize).ceil(), PhysAddr::from(MEMORY_END as usize).floor());
}