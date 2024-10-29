use core::any::Any;



/// 在 easy-fs 中并没有一个实现了 BlockDevice Trait 的具体类型。  
/// 因为块设备仅支持以块为单位进行随机读写，所以需要由具体的块设备驱动来实现这两个方法，
pub trait BlockDevice :Send + Sync + Any {
    ///  将编号为 block_id 的块从磁盘读入内存中的缓冲区 buf ；
    fn read_block(&self,block_id:usize,buf:&mut [u8]);

    /// 将内存中的缓冲区 buf 中的数据写入磁盘编号为 block_id 的块
    fn write_block(&self,block_id:usize,buf:&[u8]);
}