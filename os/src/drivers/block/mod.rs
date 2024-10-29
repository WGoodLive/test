use alloc::sync::Arc;
use easy_fs::BlockDevice;
use lazy_static::lazy_static;
pub mod virtio_blk;
pub use virtio_blk::VirtIOBlock;

use crate::board::BlockDeviceImpl;

// 在 qemu 上，我们使用 VirtIOBlock 访问 VirtIO 块设备；
#[cfg(feature = "board_qemu")]
type BlockDeviceImpl = virtio_blk::VirtIOBlock;

// 而在 k210 上，我们使用 SDCardWrapper 来访问插入 k210 开发板上真实的 microSD 卡
#[cfg(feature = "board_k210")]
type BlockDeviceImpl = sdcard::SDCardWrapper;

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}