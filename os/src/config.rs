//! Constants used in rCore

pub const USER_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const MAX_APP_NUM: usize = 4;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
pub const PAGE_SIZE: usize = 0x1000;
pub const PAGE_SIZE_BITS: usize = 0xc;

pub const VA_WIDTH_SV39: usize = 39;
pub const PA_WIDTH_SV39: usize = 56; // 因为SV39转换的物理地址是56位
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS; // 得到的就是PPN
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;
// 物理内存:[0x80000000,0x80800000)
// ekernel指名内核的终止物理地址
pub const MEMORY_END:usize = 0x80800000;

pub use crate::board::CLOCK_FREQ;

pub const TRAMPOLINE:usize = usize::MAX - PAGE_SIZE + 1;
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;
pub fn kernel_stack_position(app_id:usize)->(usize,usize){
    let top = TRAMPOLINE-app_id*(KERNEL_STACK_SIZE+PAGE_SIZE); // 这个页是guard_page,不是跳板页
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom,top)
}

