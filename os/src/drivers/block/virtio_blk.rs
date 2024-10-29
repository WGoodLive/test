use super::BlockDevice;
use crate::mm::{
    frame_alloc, frame_dealloc, kernel_token, FrameTracker, PageTable, PhysAddr, PhysPageNum,
    StepByOne, VirtAddr,
};
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use lazy_static::*;
use virtio_drivers::{Hal, VirtIOBlk, VirtIOHeader};

const VIRTIO0: usize = 0x10001000;

/// 将 virtio-drivers crate 提供的 VirtIO 块设备抽象 VirtIOBlk 包装为我们自己的 VirtIOBlock ，实质上只是加上了一层互斥锁，
/// - 下面这一部分代码的讲解，具体看ch6.md中`VirtIOBlk的讲解`这部分
pub struct VirtIOBlock(UPSafeCell<VirtIOBlk<'static, VirtioHal>>);

impl VirtIOBlock {
    #[allow(unused)]
    pub fn new() -> Self {
        unsafe {
            // VirtIOHeader记录：以 MMIO 方式访问 VirtIO 设备所需的一组设备寄存器的地址
            Self(UPSafeCell::new(
                VirtIOBlk::<VirtioHal>::new(&mut *(VIRTIO0 as *mut VirtIOHeader)).unwrap(),
            ))
        }
    }
}

/// 实现磁盘设备需要实现的接口：BlockDevice【这里直接转发VirIOBlk驱动已经实现的】
impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0.exclusive_access().read_block(block_id, buf).expect("Error when reading VirtIOBlk");
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0.exclusive_access().write_block(block_id, buf).expect("Error when writing VirtIOBlk");
    }
}





lazy_static! {
    static ref QUEUE_FRAMES: UPSafeCell<Vec<FrameTracker>> = unsafe { UPSafeCell::new(Vec::new()) };
}

/// 下面这一部分代码的讲解，具体看ch6.md中`VirtIOBlk的讲解`这部分
pub struct VirtioHal;


impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> usize {
        let mut ppn_base = PhysPageNum(0);
        for i in 0..pages {
            let frame = frame_alloc().unwrap();
            if i == 0 {
                ppn_base = frame.ppn;
            }
            assert_eq!(frame.ppn.0, ppn_base.0 + i);
            QUEUE_FRAMES.exclusive_access().push(frame);
        }
        let pa: PhysAddr = ppn_base.into();
        pa.0
    }

    fn dma_dealloc(pa: usize, pages: usize) -> i32 {
        let pa = PhysAddr::from(pa);
        let mut ppn_base: PhysPageNum = pa.into();
        for _ in 0..pages {
            frame_dealloc(ppn_base);
            ppn_base.step();
        }
        0
    }

    /// 转换一个被VirtIO使用的物理地址为进程需要的虚拟地址
    fn phys_to_virt(addr: usize) -> usize {
        addr
    }
    /// 虚拟地址转换为物理地址
    fn virt_to_phys(vaddr: usize) -> usize {
        PageTable::from_token(kernel_token())
            .translate_va(VirtAddr::from(vaddr))
            .unwrap()
            .0
    }
}