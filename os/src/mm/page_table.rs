use bitflags::*;

use alloc::vec::Vec;
use alloc::vec;
use lazy_static::lazy_static;
use crate::{mm::{address::PhysAddr, MEMORY_END}, sync::UPSafeCell, task::{change_program_sbrk, current_user_token}};

use super::{address::{PhysPageNum, StepByOne, VirtAddr, VirtPageNum}, frame_allocator::{self, frame_alloc, FrameTracker}, PA_WIDTH_SV39, PPN_WIDTH_SV39};

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

lazy_static!{
    pub static ref PAGEREFCOUNT:UPSafeCell<SharePage> = unsafe {
        UPSafeCell::new(SharePage::new())
    };
}
pub struct SharePage{
    page_ref:Vec<PhysPageNum>,
    ref_num:Vec<usize>,
}

impl SharePage {
    pub fn new()->Self{
        Self{
            page_ref:Vec::new(),
            ref_num:Vec::new(),
        }
    }

    pub fn init(&mut self,length:usize){
        self.page_ref = vec![(0 as usize).into();length];
        self.ref_num = vec![0;length];
    }

    pub fn add_page(&mut self,id:usize,vpn:VirtAddr) ->PhysPageNum{
        if(id<self.page_ref.len()){
            if(self.page_ref[id].0==0){
                let token = current_user_token();
                let entry = PageTable::from_token(token);
                entry.transform(false, vpn.into());
                let u: Option<PageTableEntry> = entry.translate(vpn.into());
                match u {
                    Some(entry) => {
                        self.ref_num[id] +=1;
                        self.page_ref[id] = entry.ppn();
                        self.page_ref[id]
                    },
                    None=>panic!("add_page is not exist...")
                }
            }else{
                self.ref_num[id] +=1;
                self.page_ref[id]
            }
        }else{
            panic!("add_page's id must be less than length...")
        }
    }

    pub fn remove_page(&mut self,id:usize) ->(usize,PhysPageNum){
        if(id<self.page_ref.len()){
            let num = self.ref_num[id];
            let ppn = self.page_ref[id];
            if(num==0){
                panic!("remove_page err...")
            }else if num==1 {
                self.page_ref[id] = (0 as usize).into();
                self.ref_num[id]=0;
                (0,ppn)
            }else{
                self.ref_num[id]-=1;
                (num-1,ppn)
            }
        }else{
            panic!("add_page's id must be less than length...")
        }
    }
}

pub fn init_ref_count(){
    PAGEREFCOUNT.exclusive_access().init(8);
}

pub fn add_count(id:usize,vpn:VirtAddr) -> PhysPageNum{
    PAGEREFCOUNT.exclusive_access().add_page(id, vpn)
}

pub fn remove_share(ppn:PhysPageNum){
    
}



/// 储存根节点，以及物理页号
pub struct PageTable{
    root_ppn:PhysPageNum,
    frames:Vec<FrameTracker>,
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
        // PAGEREFCOUNT.exclusive_access().page_ref_add(ppn);
        *pte = PageTableEntry::new(ppn, flags|PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum){
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        // PAGEREFCOUNT.exclusive_access().page_ref_reduce(pte.ppn());
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

    pub fn token(&self) ->usize{
        8usize << 60 | self.root_ppn.0
    }

    
    pub fn transform(&self,w:bool,vpn:VirtPageNum){
        let entry_w = self.find_pte(vpn).unwrap();
        let flags;
        if(w){
            flags = entry_w.flags() & PTEFlags::W;
        }else {
            flags = entry_w.flags() & !(PTEFlags::W);
        }
        *entry_w = PageTableEntry::new(entry_w.ppn(), flags);
    }
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