use buddy_system_allocator::LockedHeap;
use crate::config::KERNEL_HEAP_SIZE;

#[global_allocator]
static HEAP_ALLOCATOR:LockedHeap = LockedHeap::empty();
// LockedHeap 已经实现了 GlobalAlloc 要求的抽象接口了。
// 堆分配器

static mut HEAP_SPACE:[u8;KERNEL_HEAP_SIZE] = [0;KERNEL_HEAP_SIZE];// 单线程的；.bss段
pub fn init_heap(){
    unsafe {
        HEAP_ALLOCATOR
        .lock() //获取锁，以防其他线程竞争
        .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE) // 分配一个内存，进行分配
    }
}

//动态内存分配失败执行函数 
// 当使用alloc crate而不使用std crate时，此属性是强制性的。
#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

extern crate alloc;
#[allow(unused)]
pub fn heap_test(){
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    /* 
    由于我们申请数据的方式，可以知道，栈被放在了.bss段

    Box指针放在堆中

    数组指针放在堆中，变量放在栈中
    */
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for i in 0..500 {
        assert_eq!(v[i], i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    
}