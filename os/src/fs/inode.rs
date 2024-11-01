use alloc::vec::Vec;
use easy_fs::{EasyFileSystem, Inode};
use alloc::sync::Arc;
use crate::{mm::UserBuffer, sync::UPSafeCell};

use super::File;
use lazy_static::lazy_static;
use crate::drivers::block::BLOCK_DEVICE;

/// 文件打开标记
bitflags! {
    pub struct OpenFlags: u32 {
        // 只读
        const RDONLY = 0;
        // 只写
        const WRONLY = 1 << 0;
        // 可读可写
        const RDWR = 1 << 1;
        // 可创建（存在就覆盖）
        const CREATE = 1 << 9;
        // 打开文件的时候，先把文件内容清空
        const TRUNC = 1 << 10;
    }
}
impl OpenFlags {
    /// Return(readable,writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            // Openflags空：可读
            (true, false)
        } else if self.contains(Self::WRONLY) {
            // 只写
            (false, true)
        } else {
            // 可读可写
            (true, true)
        }
    }
}

// 打开文件系统使用
// 前面对文件系统的使用也打开过一次，那时是进行写入
lazy_static! {
    // 1. 打开块设备：BLOCK_DEVICE,这个只需要实现BlockDevice需要的驱动接口
    pub static ref ROOT_INODE: Arc<Inode> = {
        // 2. 打开文件系统
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        // 3. 获取文件系统的根目录
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// OSInode:就表示进程中一个被打开的常规文件或目录  
/// 由于不同进程的访问文件属性不同,不会在文件系统记录，所以封装一个进程使用的  
/// - 读写属性
/// - OSInodeInner(文件偏移 + 具体Inode):加上一个互斥，应对多个文件同事读取的情况
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}

/// 偏移 + Inode
pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    pub fn new(
        readable: bool,
        writable: bool,
        inode: Arc<Inode>,
    ) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe {
                UPSafeCell::new(OSInodeInner {
                    offset: 0,
                    inode,
                })
            },
        }
    }
    /// 读取所有的数据，以字节数组的形式返回
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]); // list.append 差不多
        }
        v
    }
}

impl File for OSInode {
    fn readable(&self) -> bool { self.readable }
    fn writable(&self) -> bool { self.writable }

    
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut(){
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    /// 向Inode里面写数据，buf不用可变
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}

/* ----------------------外部接口----------------------*/
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
    //     for _ in 0..3{
    //         print!("{}   ", app);
    //     }
        println!("{}   ", app);
    }
    println!("**************/")
}

/// 打开文件：
/// 1. 查看创建标志是否存在，存在就覆盖/不存在就创建文件，然后返回
/// 2. 下面分两步
/// - 没有创建标志，就查看TRUNC是否存在，存在就清空文件，否则相安无事
/// - 然后直接打开文件返回
pub fn open_file(name:&str,flags:OpenFlags) -> Option<Arc<OSInode>>{
    let (readable,writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE){
        if let Some(inode) = ROOT_INODE.find(name){
            inode.clear();
            Some(
                Arc::new(
                OSInode::new(readable, writable, inode)
                )
            )
        }else {
            ROOT_INODE.create(name)
                .map(|inode| {
                    Arc::new(OSInode::new(
                        readable,
                        writable,
                        inode,
                    ))
                })
        }
    }else{
        ROOT_INODE.find(name)
            .map(|inode| {
                if flags.contains(OpenFlags::TRUNC) {
                    inode.clear();
                }
                Arc::new(OSInode::new(
                    readable,
                    writable,
                    inode
                ))
            })
    }
}