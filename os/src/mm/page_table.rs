use bitflags::*;

use alloc::{string::String, vec::Vec};
use alloc::vec;

use super::address::PhysAddr;
use super::PPN_WIDTH_SV39;
use super::{address::{PhysPageNum, StepByOne, VirtAddr, VirtPageNum}, frame_allocator::{self, frame_alloc, FrameTracker}};

bitflags! {
    pub struct PTEFlags:u8{
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Clone,Copy)]
#[repr(C)]
/// 页表项,保存每个物理页的相关属性
pub struct PageTableEntry{
    pub bits:usize,
}

impl PageTableEntry {
    /// 对物理页设置属性，然后创建该页的页表项
    pub fn new(ppn:PhysPageNum,flags:PTEFlags) -> Self{
        Self{
            bits:ppn.0<<10 | flags.bits as usize
        }
    }

    /// 设置为全0页表项(V：0,页表项不合法)，实现清除页表项的作用
    pub fn empty() -> Self{
        Self{
            bits:0,
        }
    }

    /// 获取页表项的物理页号(PPN)
    pub fn ppn(&self) -> PhysPageNum{
        (self.bits>>10 & ((1usize << PPN_WIDTH_SV39) -1)).into()
    }

    // from_bits(一一对应设置bit)
    /// 获取页表项的标志位
    pub fn flags(&self) -> PTEFlags{
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    /// 判断页表项是否合法
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// 判断页表项是否可读
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// 判断页表项是否可写
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// 判断页表项是否可执行
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }

}

/// 储存根节点，以及物理页号
pub struct PageTable{
    root_ppn:PhysPageNum,
    frames:Vec<FrameTracker>
}
// 一个节点所在物理页帧的物理页号其实就是指向该节点的“指针”。
impl PageTable {
    pub fn new()->Self{
        let frame = frame_alloc().unwrap();
        PageTable { root_ppn: frame.ppn, frames: vec![frame] }
        // 这与frame_allocator_test(),这个测试程序是一个思路，即将这些 FrameTracker 的生命周期进一步绑定到 PageTable 下面
    }
    
    /// 虚拟页号到页表项的映射
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags){
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(),"vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags|PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum){
        let pte = self.find_pte(vpn).unwrap();
        
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    
    fn find_pte_create(&mut self,vpn:VirtPageNum) -> Option<&mut PageTableEntry>{
        let idxs = vpn.indexes(); 
        let mut ppn = self.root_ppn;
        let mut result:Option<&mut PageTableEntry> = None;
        for i in 0..3{
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i==2{
                result = Some(pte);
                break;
            }
            if !pte.is_valid(){
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 临时页框，生命周期等于from_token存在周期  
    /// 非当前正处在的地址空间的页表时
    /// 目前还没分不同应用的页表
    pub fn from_token(stap:usize)->Self{
        Self { root_ppn: PhysPageNum::from(stap & ((1<<44)-1)), frames: Vec::new(), }
    }

    /// 复制页表，只要有地址就行
    pub fn translate(&self,vpn:VirtPageNum) ->Option<PageTableEntry>{
        self.find_pte(vpn).map(|pte|{pte.clone()})
    }

    // 虚拟地址转化为真实物理页地址
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into() 
        })
    }

    pub fn token(&self) ->usize{
        8usize << 60 | self.root_ppn.0
    }
}

pub fn translated_str(token:usize,ptr:*const u8)->String{
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop{
        let ch: u8 = *(page_table.translate_va(VirtAddr::from(va)).unwrap().get_mut());
         if ch==0{
            break;
         }else{
            string.push(ch as char);
            va +=1
         }
    }
    string
}

pub fn translated_byte_buffer(
    token: usize,
    ptr: *const u8,
    len: usize
) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table
            .translate(vpn)
            .unwrap()
            .ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

///translate a generic through page table and return a mutable reference
pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_mut()
}

/// ------用户地址空间的文件缓存(由于都储存在内存中),一个文件一个UserBufffer,-----   
/// **上面的理解出问题了**，  
/// 这个结构体就是对File接口实现写入数据的一种抽象
/// 用户的缓存就是文件描述符，跟这个没关系，目前来说
pub struct UserBuffer{
    pub buffers:Vec<&'static mut [u8]>
}

impl UserBuffer {
    pub fn new(buffers: Vec<&'static mut [u8]>) -> Self {
        Self { buffers }
    }
    pub fn len(&self) -> usize {
        let mut total: usize = 0;
        for b in self.buffers.iter() {
            total += b.len();
        }
        total
    }
}

pub struct UserBufferIterator {
    buffers: Vec<&'static mut [u8]>,
    current_buffer: usize, // 当前是第几块缓存
    current_idx: usize,     // 当前的缓存的数据指针
}

/// 一次读取一个u8数据
impl IntoIterator for UserBuffer{
    type Item = *mut u8;
    type IntoIter = UserBufferIterator;

    fn into_iter(self) -> Self::IntoIter {
        UserBufferIterator {
            buffers: self.buffers,
            current_buffer: 0,
            current_idx: 0,
        }
    }
}

impl Iterator for UserBufferIterator {
    type Item = *mut u8;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_buffer >= self.buffers.len() {
            None
        } else {
            let r = &mut self.buffers[self.current_buffer][self.current_idx] as *mut _;
            if self.current_idx + 1 == self.buffers[self.current_buffer].len() {
                self.current_idx = 0;
                self.current_buffer += 1;
            } else {
                self.current_idx += 1;
            }
            Some(r)
        }
    }
}