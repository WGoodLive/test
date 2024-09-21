pub mod heap_allocator;
pub mod address;
pub mod page_table;
pub mod frame_allocator;
// pub mod memory_set;
pub mod memory_set;
pub use crate::config::*;

pub use memory_set::KERNEL_SPACE;

pub fn init(){
    heap_allocator::init_heap();

    frame_allocator::init_frame_allocator();

    KERNEL_SPACE.exclusive_access().activate();
    // UPSafeCell让他可变
    // Arc实现了Deref使他可以自动解引用
    // RefMut<'_,T>让他被释放之后，自动释放Arc的互斥锁
}