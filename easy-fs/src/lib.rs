#![no_std]
extern crate spin;
extern crate alloc;
extern crate lazy_static;
pub const BLOCK_SZ:usize = 512;

mod block_cache;
mod block_dev;
mod layout;
mod bitmap;
mod efs;
mod vfs;

pub use block_dev::BlockDevice;
pub use efs::EasyFileSystem;
pub use vfs::Inode;