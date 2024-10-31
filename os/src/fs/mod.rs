pub mod inode;
pub mod stdio;
pub mod pipe;
pub use stdio::{Stdin,Stdout};


pub use inode::{list_apps, open_file, OSInode, OpenFlags};

use crate::mm::UserBuffer;
/// 数据的抽象，方便进程通过简洁的统一接口访问数据  
/// 接口使内存和存储设备之间建立了数据交换的通道  
/// UserBuffer：是用户地址空间的文件缓存
pub trait File : Send + Sync {
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;

    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
}



