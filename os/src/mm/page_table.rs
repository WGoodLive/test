use bitflags::*;
use riscv::addr::PhysAddr;
use alloc::vec::Vec;
use alloc::vec;

use super::{address::{PhysPageNum, VirtPageNum}, frame_allocator::{self, frame_alloc, FrameTracker}, PA_WIDTH_SV39, PPN_WIDTH_SV39};

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
/// 页表项
pub struct PageTableEntry{
    pub bits:usize,
}

impl PageTableEntry {
    pub fn new(ppn:PhysPageNum,flags:PTEFlags) -> Self{
        Self{
            bits:ppn.0<<10 | flags.bits as usize
        }
    }

    /// 设置为全0页表项(V：0,页表项不合法)
    pub fn empty() -> Self{
        Self{
            bits:0,
        }
    }

    /// 获取物理页号(PPN)
    pub fn ppn(&self) -> PhysPageNum{
        (self.bits>>10 & ((1usize << PPN_WIDTH_SV39) -1)).into()
    }

    // from_bits(一一对应设置bit)
    /// 获取标志位
    pub fn flags(&self) -> PTEFlags{
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    /// 页表项是否合法
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// 是否可读
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// 是否可写
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// 是否可执行
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
}
