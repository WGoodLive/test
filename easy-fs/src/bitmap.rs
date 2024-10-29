use alloc::sync::Arc;

use crate::{block_cache::get_block_cache, block_dev::BlockDevice, BLOCK_SZ};


const BLOCK_BITS: usize = BLOCK_SZ * 8;
/// 这个是磁盘数据结构中,位图区域数据
type BitmapBlock = [u64; 64]; // 64*64 = 4096bits

/// Return (block_id, bits64_pos, inner_pos)
fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit = bit % BLOCK_BITS;
    (block_pos, bit / 64, bit % 64)
}

/// 记录它所在区域的起始块编号以及区域的长度为多少个块  
/// 注意 Bitmap 自身是驻留在内存中的，但是它能够表示索引节点/数据块区域中的那些磁盘块的分配情况
pub struct Bitmap{
    start_block_id:usize,
    blocks:usize
}

impl Bitmap {
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    // 在给定的块设备上分配一个块
    pub fn alloc(&self,block_device:&Arc<dyn BlockDevice>) -> Option<usize>{
        // 遍历所有块
        for block_id in 0..self.blocks{ 
            
            let pos = get_block_cache(
                block_id+self.start_block_id as usize, 
                Arc::clone(block_device)
            ).lock()
            // 修改第 block_id + self.start_block_id的块
            .modify(
                0, // 从0开始读，读BitmapBlock(4096bits)的数据大小 
                |bitmap_block:&mut BitmapBlock|{ // 这里泛型T解析为&mut BitmapBlock
                    // 遍历位图块
                    if let Some((bits64_pos,inner_pos)) = bitmap_block.iter().enumerate()
                    .find(|(_,bits64)|**bits64 != u64::MAX)// 如果不是全1
                    .map(|(bits64_pos,bits64)|{ 
                        // 找到第一个未分配的位
                        (bits64_pos,bits64.trailing_ones() as usize) // 找到这个字节，和这个字节尾随的1数目
                    })
                    {
                        // 将该位设置为已分配
                        // u64的数据
                        bitmap_block[bits64_pos] |= 1u64<<inner_pos;
                        // 返回该块的索引，一位就是一个索引
                        Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize) 
                    }else {
                        // 如果所有位都已被分配，则返回None
                        None
                    }
                }
            );
            // 如果找到了一个未分配的块，则返回该块的索引
            if pos.is_some(){
                return pos;
            }
        }
        // 如果所有块都已被分配，则返回None
        None
    }

    pub fn dealloc(&self,block_device:&Arc<dyn BlockDevice>,bit:usize){
        let (block_id, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(block_id+self.start_block_id, Arc::clone(block_device))
        .lock().modify(0, |bitmap_block: &mut BitmapBlock|{
            assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
            bitmap_block[bits64_pos] -= 1u64 << inner_pos; // 0b1<<2 = 0b100
        });
    }

    /// bits容量
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}
