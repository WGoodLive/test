//! 服务于文件相关系统调用的索引节点层的代码在 vfs.rs 中
//! EasyFileSystem 实现了磁盘布局并能够将磁盘块有效的管理起来。
//! 但是对于文件系统的使用者而言，他们往往不关心磁盘布局是如何实现的，而是更希望能够直接看到目录树结构中逻辑上的文件和目录。

use alloc::{string::String, sync::Arc, vec::Vec};
use spin::{Mutex, MutexGuard};
use crate::{block_cache::{block_cache_sync_all, get_block_cache}, block_dev::BlockDevice, efs::EasyFileSystem, layout::{DirEntry, DiskInode, DiskInodeType, DIRENT_SZ}};


pub struct Inode {
    // DiskInode = (block_id,block_offset)
    block_id: usize, 
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// 对Inode相应磁盘的DiskInode节点进行函数f读取处理，得到返回值
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(
            self.block_id,
            Arc::clone(&self.block_device)
        ).lock().read(self.block_offset, f)
    }

    /// 对Inode相应磁盘的DiskInode节点进行函数f修改处理，得到返回值
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(
            self.block_id,
            Arc::clone(&self.block_device)
        ).lock().modify(self.block_offset, f)
    }
    
    /// 新建，顺便初始化，  
    /// 不会在调用 Inode::new 过程中尝试获取整个 EasyFileSystem 的锁来查询 inode 在块设备中的位置，
    /// 而是在调用它之前预先查询并作为参数传过去。也即新建之前需要先有节点在磁盘中
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    )->Self{
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// 1. Inode就是封装给操作系统的，没吊用  
    /// 2. 先找到DiskInode
    /// 3. 在DiskInode里面找DirEntry，进行文件名比对，
    /// 4. 获得相应：文件/文件夹的DiskInode。里面存储的有效信息  
    /// 因为是扁平操作系统，只有根目录会调用这个
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(
            |disk_inode| {
                self.find_inode_id(name, disk_inode)
                .map(|inode_id| {
                    let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                    Arc::new(Self::new(
                        block_id,
                        block_offset,
                        self.fs.clone(),
                        self.block_device.clone(),
                    ))
                })
            }
        )
    }
    /// 从这里可以看出来，文件跟文件夹，操作系统都当Inode处理的，然后都是找DirEntry
    /// 返回：从当前文件夹的DirEntry一次一次读,知道读取到目标文件夹，返回目标文件夹的索引
    fn find_inode_id(
        &self,
        name: &str,
        disk_inode: &DiskInode,
    ) -> Option<u32> {
        // 首先，看看这个disk_node是一个文件夹吗
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(
                    DIRENT_SZ * i,
                    dirent.as_bytes_mut(),
                    &self.block_device,
                ),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_number() as u32);
            }
        }
        None
    }

    /// 返回当前目录下的所有文件  
    /// 目前就根目录会调用
    pub fn ls(&self) -> Vec<String> {
        // _name:下划线的开始变量不访问，不会警告(不使用，就是为了开个锁而已，尽管不用)
        let _fs = self.fs.lock();
        
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(
                        i * DIRENT_SZ,
                        dirent.as_bytes_mut(),
                        &self.block_device,
                    ),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }

    /// 先通过位图分配处块，然后增加块在当前的Inode的数据部分长度
    /// 位图分配不区分是索引还是数据，只负责给出  
    /// **数据块之间不一定连续，同一Inode的也不一定连续**
    /// 注意，这里的增加是说，你在写入的时候，先执行这个函数，
    /// - 如果写入长度不够长，就不用增加；
    /// - 如果写入长度打了，就申请新的空号页
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>, // 这是一种状态，表示传的是lock()后的EasyFileSystem
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data()); // 一级索引啥的，算数据不算Inode位图区
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }

    /// 创建文件
    /// Inode按块储存...
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        // 从当前的(block_id,block_offset)的inode出发
        if self.modify_disk_inode(|root_inode| {
            assert!(root_inode.is_dir());
            // 看看文件是不是已经创建了
            self.find_inode_id(name, root_inode)
        }).is_some() {
            return None;
        }
        // 创建文件的Inode
        let new_inode_id = fs.alloc_inode();
        // 获取Inode的编号，初始化
        let (new_inode_block_id, new_inode_block_offset)
            = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(
            new_inode_block_id as usize,
            Arc::clone(&self.block_device)
        ).lock().modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
            new_inode.initialize(DiskInodeType::File); // 初始化为文件
        });
        self.modify_disk_inode(|root_inode| {
            
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // 先把目录的文件数修改了，然后把需要的块提前分配好
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // 然后把新的文件加入目录的DiskInode中
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        // 获取新的块，进行返回
        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // 自动释放efs.lock()
    }

    /// 释放文件
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
                // 把DiskInode名下的所有块全部回收了，Diskinode还在(自我删除，目前不允许)
            }
        });
        block_cache_sync_all();
    }

    /// 从当前Inode中从offset读取buf.len()长度的数据
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// 向当前Inode中从offset写入buf.len()长度数据
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
}
