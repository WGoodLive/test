const BLOCK_CACHE_SIZE: usize = 16;


use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::{block_dev::BlockDevice, BLOCK_SZ};


lazy_static!{
    pub static ref BLOCK_CACHE_MANAGER:Mutex<BlockCacheManager> = Mutex::new(
        BlockCacheManager::new()
    );
}


pub fn get_block_cache(block_id:usize,block_device:Arc<dyn BlockDevice>)->Arc<Mutex<BlockCache>>{
    BLOCK_CACHE_MANAGER.lock().get_block_cache(block_id, block_device) // 你要修改他，所以要解锁
}

/// 同步缓存给磁盘
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}

/// block_device是一个底层块设备的引用，可通过它进行块读写；
pub struct BlockCache{
    cache:[u8;BLOCK_SZ],
    block_id:usize,
    block_device:Arc<dyn BlockDevice>,
    modified:bool,
}

impl BlockCache {
    pub fn new(block_id:usize,block_device:Arc<dyn BlockDevice>)->Self{
        let mut cache = [0u8;BLOCK_SZ];
        block_device.read_block(block_id,&mut cache);
        Self { 
            cache, 
            block_id, 
            block_device, 
            modified:false
        }
    }
    /// 返回偏移处的指针，
    /// - **错误**：磁盘数据的地址？
    /// - **正确**：谁创建的这个结构体，然后返回创建处的数据的地址
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize // 
    }
    /// 返回数据T的引用  
    /// - 这里编译器会自动进行生命周期标注
    /// 约束返回的引用的生命周期不超过 BlockCache 自身，在使用的时候我们会保证这一点。
    pub fn get_ref<T>(&self, offset: usize) -> &T 
    where T: Sized 
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset+type_size<=BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe {
            &(*(addr as *const T)) 
            // 获得addr的T类型数据的引用
            // 然后取出数据，再加不可变引用
        }
    }

    /// 返回数据T的可变引用   
    /// - 这里编译器会自动进行生命周期标注
    /// 约束返回的引用的生命周期不超过 BlockCache 自身，在使用的时候我们会保证这一点。  
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T where T: Sized {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    pub fn sync(&mut self){
        if self.modified{
            self.modified=false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }

    /// 这个函数的用途是读取存储在某个位置的数据，并使用一个闭包来读取这些数据，执行一些操作。
    /// 类似于你给self又实现了一个方法，让一个与BlockCache无关的函数能更顺畅的执行  
    /// - **闭包可以获得调用他的元素的执行环境**
    /// - FnOnce是获取所有权(不用考虑可变不可变)
    pub fn read<T,V>(&self,offset: usize,f:impl FnOnce(&T)->V) -> V{
        f(self.get_ref(offset))
    }
    /// 这个函数的用途是获取在某个位置的数据，并使用一个闭包来处理这些数据，，执行一些操作。
    /// 类似于你给self又实现了一个方法，让一个与BlockCache无关的函数能更顺畅的执行  
    /// - **闭包可以获得调用他的元素的执行环境**
    /// - FnOnce是获取所有权(不用考虑可变不可变)
    pub fn modify<T, V>(&mut self, offset:usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}


/// 全局缓存管理器
/// 只针对同一个BlockDevice
/// - queue:VecDeque<(usize,Arc<Mutex<BlockCache>>)>:usize是块编号
pub struct BlockCacheManager{
    queue:VecDeque<(usize,Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {

    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// 指定的块 = 设备 + 块编号
    pub fn get_block_cache(
        &mut self,
        block_id:usize,
        block_device:Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>>{
        if let Some(pair) = self.queue
        .iter()
        .find(|pair|pair.0 == block_id){
            Arc::clone(&pair.1) // 返回的强引用
        }else{
            // 替换
            if self.queue.len() == BLOCK_CACHE_SIZE{
                if let Some((idx,_)) = self.queue.iter()
                .enumerate().find(
                    |(_,pair)|Arc::strong_count(&pair.1)==1
                ){
                    // drain方法会返回一个迭代器，这个迭代器可以用来遍历被移除的元素，但不会返回被移除的元素本身
                    self.queue.drain(idx..=idx); // 这时候，会自己前移，中间不会留空
                }else {
                    panic!("Run out of BlockCache!")
                }
            }

            let block_cache = Arc::new(
                Mutex::new(
                    BlockCache::new(
                        block_id,
                        Arc::clone(&block_device)
                    )
                )
            );
            self.queue.push_back((block_id,Arc::clone(&block_cache)));
            block_cache
        }
    }
}