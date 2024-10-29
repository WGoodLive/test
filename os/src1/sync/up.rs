use core::cell::{RefCell, RefMut};


// newtype
// 此时我们仍然是单线程的任务
// 不会涉及多线程问题
// 在这里我们为了骗编译器，我的RefCell是线程安全的，所以newtype
/// 这里需要保证
/// 1. 单线程(多线程不安全)
/// 2. 为了让编译器认为RefCell是多线程安全的，带来的代价:
/// 同一时间，只能有一个所有者，不论你是读还是写
pub struct UPSafeCell<T>{
    inner:RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T>{}
    

impl<T> UPSafeCell<T>{

    /// 可能悬垂引用，所以不安全
    pub unsafe fn new(value:T) ->Self{ //当前类型(实例)
        //1. 在Rust中，Self是一个类型，它表示当前类型的实例。
        //2. 在类和trait的定义中，我们可以使用Self来表示当前类型。
        Self{inner:RefCell::new(value)}// 当前类型
    }

    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> RefMut<'_,T>{// '_：泛型生命周期
        self.inner.borrow_mut()
    }

    
}