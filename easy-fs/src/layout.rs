use alloc::{sync::Arc, vec::Vec};

use crate::{block_cache::get_block_cache, block_dev::BlockDevice, BLOCK_SZ};


/// Magic number for sanity check
const EFS_MAGIC: u32 = 0x3b800001;
const INODE_DIRECT_COUNT: usize = 28;
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;
const NAME_LENGTH_LIMIT: usize = 27;
pub const DIRENT_SZ: usize = 32;

type IndirectBlock = [u32; BLOCK_SZ / 4];


/// 更上层的磁盘块管理器需要完成的工作,  
/// 就存放在磁盘上编号为 0 的块的起始处。
#[repr(C)]
pub struct SuperBlock{
    magic:u32,// 魔数，判断是否合法
    pub total_blocks:u32, // 总块数：但是这并不等同于所在磁盘的总块数
    pub inode_bitmap_blocks:u32,
    pub inode_area_blocks:u32,
    pub data_bitmap_blocks:u32,
    pub data_area_blocks:u32,
}

impl SuperBlock{
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self{
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

/// 索引的节点类型
#[derive(PartialEq)] // 可以比较
pub enum DiskInodeType {
    File,
    Directory,
}

/// 文件/目录在磁盘上均以一个 DiskInode 的形式存储
#[repr(C)]
pub struct DiskInode{
    pub size: u32, // 文件或者目录的内容字节数
    // 存储文件/目录的实际数据的索引
    pub direct: [u32; INODE_DIRECT_COUNT], // 直接索引
    pub indirect1: u32, // 间接索引
    pub indirect2: u32,
    type_: DiskInodeType, 
}

impl DiskInode {
    pub fn initialize(&mut self,type_:DiskInodeType){
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }

    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }
    /// 取出文件的第inner_id块数据块的在磁盘中的块索引
    pub fn get_block_id(&self,inner_id:u32,block_device:&Arc<dyn BlockDevice>)->u32{
        let inner_id = inner_id as usize;
        if inner_id< INODE_DIRECT_COUNT{
            self.direct[inner_id]
        }else if inner_id < INDIRECT1_BOUND{
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        }else{
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(
                self.indirect2 as usize,
                Arc::clone(block_device)
            )
            .lock()
            .read(0, |indirect2: &IndirectBlock| {
                indirect2[last / INODE_INDIRECT1_COUNT]
            });
            get_block_cache(
                indirect1 as usize,
                Arc::clone(block_device)
            )
            .lock()
            .read(0, |indirect1: &IndirectBlock| {
                indirect1[last % INODE_INDIRECT1_COUNT]
            })
        }
    }
    /// 数据块的数目，不包含索引块
    pub fn data_blocks(&self)-> u32{
        Self::_data_blocks(self.size)
    }

    fn _data_blocks(size:u32) -> u32{
        (size + BLOCK_SZ as u32 - 1) / BLOCK_SZ as u32
    }


    pub fn total_blocks(size:u32) ->u32{
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks as usize;
        if data_blocks > INODE_DIRECT_COUNT{
            total +=1;
        }
        if data_blocks > INDIRECT1_BOUND{
            total +=1;
            total +=(data_blocks - INDIRECT1_BOUND + INODE_INDIRECT1_COUNT-1)/INODE_INDIRECT1_COUNT;
        }
        total as u32
    }

    /// 拓展到new_size,额外需要的块数
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }

    pub fn increase_size(
        &mut self,
        new_size: u32, // 新的文件大小
        new_blocks: Vec<u32>, // 新插入的块
        block_device: &Arc<dyn BlockDevice>,
    ){
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();// 移出元素所有权，按照迭代的形式
        // 先填充直接索引，如果直接索引之前被填满，就不填充直接索引了
        while current_blocks < total_blocks.min(INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        // 直接索引不够
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 { // 如果之前直接索引恰好够用，要申请一级索引页
                self.indirect1 = new_blocks.next().unwrap();
            }
            // 去掉直接索引数据块，还有多少页
            current_blocks -= INODE_DIRECT_COUNT as u32; 
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else { // 直接索引够用 直接可以退出了
            return;
        }
        // 一级索引插入
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < total_blocks.min(INODE_INDIRECT1_COUNT as u32) {
                    indirect1[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        // alloc indirect2
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        } else {
            return;
        }
        // fill indirect2 from (a0, b0) -> (a1, b1)
        let mut a0 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        let mut b0 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        let a1 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        let b1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        // 填充，一次就处理叶哥数据页
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                while (a0 < a1) || (a0 == a1 && b0 < b1) {
                    if b0 == 0 { //二级索引的一级索引需要申请页
                        indirect2[a0] = new_blocks.next().unwrap();
                    }
                    // 填充
                    get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            indirect1[b0] = new_blocks.next().unwrap();
                        });
                    // 下一个
                    b0 += 1;
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            });
    }

    // 文件尺寸置0,并返回回收的块
    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32>{
        let mut v: Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        let mut current_blocks = 0usize;
        // 直接索引
        // 注意直接索引那个数据页不能收回，他是在DiskInode数据结构中的
        while current_blocks < data_blocks.min(INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0; // 索引置0
            current_blocks += 1; // 已经回收几个数据页了
        }
        // 一级索引
        if data_blocks > INODE_DIRECT_COUNT {
            v.push(self.indirect1); // 一级索引页收回(copy -> u32)
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        } else { // 直接索引够用
            return v;
        }
        // indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < data_blocks.min(INODE_INDIRECT1_COUNT) {
                    v.push(indirect1[current_blocks]);
                    // 页面后面会随着self.indirect1的弹出，给外界清0，你只需要统计哪些页
                    //indirect1[current_blocks] = 0;
                    current_blocks += 1;
                }
            });
        self.indirect1 = 0; // 结构体中数值置0
        // 二级索引
        if data_blocks > INODE_INDIRECT1_COUNT {
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            return v;
        }
        // indirect2
        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let a1 = data_blocks / INODE_INDIRECT1_COUNT;// 从0开始计数的
        let b1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                // full indirect1 blocks
                for entry in indirect2.iter_mut().take(a1) { // iterator.take(n)：取[0,n)的迭代对象
                    v.push(*entry); // 二级中的一级索引删除
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }
                // 最后一个一级块
                if b1 > 0 {
                    v.push(indirect2[a1]);
                    get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(b1) {
                                v.push(*entry);
                            }
                        });
                }
            });
        self.indirect2 = 0;
        v
    }

    /// 功能：从文件的offset字节开始读入buf.len()个字节  
    /// 返回：有效的读取字节数
    pub fn read_at(
        &self,
        offset:usize,
        buf:&mut [u8],
        block_device: &Arc<dyn BlockDevice>
    )->usize{
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        if start  >= end{ // 说明offset > self.size 
            return 0;
        }
        // 从那个块数据块开始读
        let mut start_block =  start / BLOCK_SZ;
        let mut read_size = 0usize;

        loop {
            // 计算数据在，当前块的结尾字节地址
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            // 需要读多少个字节
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];// 从0位置开始写入
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .read(0, |data_block: &DataBlock| {
                let src = &data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size];
                dst.copy_from_slice(src);
            });
            read_size += block_read_size;
            // move to next block
            if end_current_block == end { break; }
            start_block += 1;
            start = end_current_block; // 结尾即下一页的开始
        }
        read_size
    }

    /// 功能：从文件的offset字节开始写入buf.len()个字节  
    /// 返回：有效的写入字节数  
    /// 如果写入失败，直接**报错**，因为操作者在写入之前应该保证能有效写入
    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        // 与read_at完全相反的过程，没啥讲解的
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size as usize);
        assert!(start <= end);
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop {
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = end_current_block.min(end);
            let block_write_size = end_current_block - start;
            get_block_cache(
                self.get_block_id(start_block as u32, block_device) as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                let src = &buf[write_size..write_size + block_write_size];
                let dst = &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                dst.copy_from_slice(src);
            });
            write_size += block_write_size;
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
    }
}

/// 数据项
/// 每个数据页只需要字节序列
type DataBlock = [u8; BLOCK_SZ];


/// 目录项(文件与文件夹都用这个)
/// - 每个目录项占32个字节,目录项的在数据页的格式比较特殊  
/// - 文件夹的名字最多27char，然后留一个 `\0`
#[repr(C)]
pub struct DirEntry{
    name:[u8;NAME_LENGTH_LIMIT+1],
    inode_number:u32, // 索引
}

impl DirEntry {
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }

    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8;NAME_LENGTH_LIMIT+1];
        assert!(name.len()<=NAME_LENGTH_LIMIT,"directory name is invalid");
        let _ = &mut bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }

    /// 转换目录项为不可变字符数组  
    /// 由于read/write_at接口传入的是字节数组，所以要进行转换
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const _ as usize as *const u8, // 先转换成地址，然后转换为u8类型的指针
                DIRENT_SZ,
            )
        }
    }

    /// 转换目录项为可变字符数组  
    /// 由于read/write_at接口传入的是字节数组，所以要进行转换
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self as *mut _ as usize as *mut u8,
                DIRENT_SZ,
            )
        }
    }
    /// 返回文件夹名字
    pub fn name(&self) -> &str{
        let len = (0..).find(|i|self.name[*i]==0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }

    /// 返回文件夹的索引
    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}