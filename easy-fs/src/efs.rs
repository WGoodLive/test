//! 只有磁盘数据结构并没有体现出磁盘布局上各个区域是如何划分的。  
//! 实现 easy-fs 的整体磁盘布局，将各段区域及上面的磁盘数据结构结构整合起来就是简易文件系统 EasyFileSystem 的职责。  
//! 它知道每个布局区域所在的位置，磁盘块的分配和回收也需要经过它才能完成，  
//! 因此某种意义上讲它还可以看成一个磁盘块管理器
//! **注意从这一层开始，所有的数据结构就都放在内存上了**

use alloc::sync::Arc;
use block_dev::BlockDevice;
use bitmap::Bitmap;
use spin::Mutex;
type DataBlock = [u8; BLOCK_SZ];

use crate::{block_cache::{block_cache_sync_all, get_block_cache}, layout::{DiskInode, DiskInodeType, SuperBlock}, vfs::Inode, BLOCK_SZ};

/// superBlock -> inode_bitmap -> inode_area_start_block -> data_bitmap -> data_area_start_block
pub struct EasyFileSystem {
    pub block_device: Arc<dyn BlockDevice>,
    pub inode_bitmap: Bitmap,
    pub data_bitmap: Bitmap,
    inode_area_start_block: u32,
    data_area_start_block: u32,
}

impl EasyFileSystem {
    pub fn create(
        block_device: Arc<dyn BlockDevice>,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
    )-> Arc<Mutex<Self>>{
        // 开始块1,然后连续存
        let inode_bitmap = Bitmap::new(1, inode_bitmap_blocks as usize);
        // 最大用来索引的块数
        let inode_num = inode_bitmap.maximum();
        // 位图对应的索引,需要的总块数
        let inode_area_blocks =
            ((inode_num * core::mem::size_of::<DiskInode>() + BLOCK_SZ - 1) / BLOCK_SZ) as u32;
        // 位图与相应索引总共需要的块数
        let inode_total_blocks = inode_bitmap_blocks + inode_area_blocks;

        // 数据页总块数 = 总块数 - 1(超级块) -索引总块数
        let data_total_blocks = total_blocks - 1 - inode_total_blocks;
        // 索引位图块x*4096 > total - x =>  x>= ceil(y/4097)
        let data_bitmap_blocks = (data_total_blocks + 4096) / 4097;
        // 数据页
        let data_area_blocks = data_total_blocks - data_bitmap_blocks;
        
        let data_bitmap = Bitmap::new(
            (1 + inode_bitmap_blocks + inode_area_blocks) as usize,
            data_bitmap_blocks as usize,
        );

        /*
        上方代码的功能：
        superBlock -> inode_bitmap -> inode_area_start_block -> data_bitmap -> data_area_start_block
        1. 分别求出每部分需要的块数
        2. 把*_bitmap标注为位图形式（有自己的方法操作空间，方便）
        */

        let mut efs = Self {
            block_device: Arc::clone(&block_device),
            inode_bitmap,
            data_bitmap,
            inode_area_start_block: 1 + inode_bitmap_blocks,
            data_area_start_block: 1 + inode_total_blocks + data_bitmap_blocks,
        };

        // 把所有块先当成数据块清0
        for i in 0..total_blocks {
            get_block_cache(
                i as usize,
                Arc::clone(&block_device)
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                for byte in data_block.iter_mut() { *byte = 0; }
            });
        }

        // 先给第一页，就是索引0的页，作为超级块初始化
        get_block_cache(0, Arc::clone(&block_device))
        .lock()
        .modify(0, |super_block: &mut SuperBlock| {
            super_block.initialize( // 才用24个字节，后期有属性可以加在里面
                total_blocks,
                inode_bitmap_blocks,
                inode_area_blocks,
                data_bitmap_blocks,
                data_area_blocks,
            );
        });

        
        
        assert_eq!(efs.alloc_inode(), 0);
        // 获取索引为0的inode储存的地址(从超级块后面的*块*开始)
        let (root_inode_block_id, root_inode_offset) = efs.get_disk_inode_pos(0);
        get_block_cache(
            root_inode_block_id as usize,
            Arc::clone(&block_device)
        )
        .lock()
        .modify(root_inode_offset, |disk_inode: &mut DiskInode| {
            disk_inode.initialize(DiskInodeType::Directory); 
            // 首先第一个Inode作为文件夹，根目录
            // 目前他的大小，各级参数都是0
        });
        block_cache_sync_all();
        Arc::new(Mutex::new(efs))
    }

    /// 在索引位图区申请，申请块
    /// 位图只是判断有效性的，空间分配还是disk_inode处理  

    pub fn alloc_inode(&mut self) -> u32 {
        self.inode_bitmap.alloc(&self.block_device).unwrap() as u32
    }
    /// 在数据位图区申请，申请块
    /// 位图只是判断有效性的，空间分配还是disk_inode处理  
    /// 返回值是块编号！
    pub fn alloc_data(&mut self) -> u32 {
        self.data_bitmap.alloc(&self.block_device).unwrap() as u32 + self.data_area_start_block
    }

    /// 回收数据页，注意是！数据页的绝对id
    /// - 清0
    /// - 回收数据位图相应有效位
    /// - dealloc_inode 未实现，因为现在还不支持文件删除。
    pub fn dealloc_data(&mut self, block_id: u32) {
        get_block_cache(
            block_id as usize,
            Arc::clone(&self.block_device)
        )
        .lock()
        .modify(0, |data_block: &mut DataBlock| {
            data_block.iter_mut().for_each(|p| { *p = 0; })
        });
        self.data_bitmap.dealloc(
            &self.block_device,
            (block_id - self.data_area_start_block) as usize
        )
    }



    /// 通过Inode索引获得(索引块号，索引块内偏移)
    pub fn get_disk_inode_pos(&self, inode_id: u32) -> (u32, usize) {
        let inode_size = core::mem::size_of::<DiskInode>();
        let inodes_per_block = (BLOCK_SZ / inode_size) as u32;
        let block_id = self.inode_area_start_block + inode_id / inodes_per_block;
        (
            block_id,
            (inode_id % inodes_per_block) as usize * inode_size,
        )
    }

    /// 返回数据块相对id的块绝对id
    pub fn get_data_block_id(&self, data_block_id: u32) -> u32 {
        self.data_area_start_block + data_block_id
    }

    /// 打开这个磁盘设备，位图又初始化了一遍，不理解  
    /// 位图就是一个很小的结构体，他不涉及空间，只涉及改动
    pub fn open(block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        // 读超级块
        get_block_cache(0, Arc::clone(&block_device))
            .lock()
            .read(0, |super_block: &SuperBlock| {
                assert!(super_block.is_valid(), "Error loading EFS!");
                let inode_total_blocks =
                    super_block.inode_bitmap_blocks + super_block.inode_area_blocks;
                let efs = Self {
                    block_device,
                    inode_bitmap: Bitmap::new(
                        1,
                        super_block.inode_bitmap_blocks as usize
                    ),
                    data_bitmap: Bitmap::new(
                        (1 + inode_total_blocks) as usize,
                        super_block.data_bitmap_blocks as usize,
                    ),
                    inode_area_start_block: 1 + super_block.inode_bitmap_blocks,
                    data_area_start_block: 1 + inode_total_blocks + super_block.data_bitmap_blocks,
                };
                Arc::new(Mutex::new(efs))
            })
    }

    /// 因为简单文件系统仅仅支持绝对路径，所以每次都要从根目录下索引
    /// 功能：返回根的索引Inode
    pub fn root_inode(efs: &Arc<Mutex<Self>>) -> Inode {
        let block_device = Arc::clone(&efs.lock().block_device);
        // acquire efs lock temporarily
        let (block_id, block_offset) = efs.lock().get_disk_inode_pos(0);
        // release efs lock
        Inode::new(
            block_id,
            block_offset,
            Arc::clone(efs),
            block_device,
        )
    }
}