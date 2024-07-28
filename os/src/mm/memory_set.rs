use alloc::{collections::BTreeMap, vec::Vec};
use crate::{config::PAGE_SIZE, println};
use super::{address::{PhysPageNum, VPNRange, VirtAddr, VirtPageNum}, frame_allocator::{frame_alloc, FrameTracker},page_table::{PTEFlags, PageTable}};
#[derive(Copy,Clone,PartialEq,Debug)]
/// 页面映射方式
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    /// 逻辑段的访问方式
    /// PTEFlags的子集，其他标志位不保存(硬件转换用的)
    pub struct MapPermission:u8{
        const R = 1<<1;
        const W = 1<<2;
        const X = 1<<3;
        const U = 1<<4;
    }
}

/// 逻辑段：相同访问方式的一段连续地址的虚拟内存地址空间段
pub struct MapArea{
    // 逻辑段地址 + 长度(迭代器)
    vpn_range : VPNRange,
    // 当MapType是Framed时：页面映射
    data_frames:BTreeMap<VirtPageNum,FrameTracker>,
    // 映射方式
    map_type:MapType,
    // 访问方式
    map_perm:MapPermission
}

impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }

    pub fn map_one(&mut self,page_table: &mut PageTable,vpn:VirtPageNum){
        let ppn:PhysPageNum;
        match self.map_type {
            MapType::Identical=>{
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed=>{
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }

    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        match self.map_type {
            MapType::Framed => {
                self.data_frames.remove(&vpn);
            }
            _ => {}
        }
        page_table.unmap(vpn);
    }
}

/// 任务的地址空间
pub struct MemorySet{
    page_table:PageTable,
    areas:Vec<MapArea>,
}

impl MemorySet{
    pub fn new_bare()->Self{
        Self { page_table: PageTable::new(), areas: Vec::new() }
    }

    /// 在当前地址空间插入一个新的逻辑段 map_area ，  
    /// 如果它是以 Framed 方式映射到物理内存，还可以可选地在那些被映射到的物理页帧上写入一些初始化数据 data ；
    fn push(&mut self,mut map_area:MapArea,data:Option<&[u8]>){
        map_area.map(&mut self.page_table);
        if let Some(data)=data{
            map_area.copy_data(&self.page_table,data);
        }
        self.areas.push(map_area);
    }

    /// 注意该方法的调用者要保证同一地址空间内的任意两个逻辑段不能存在交集
    pub fn insert_framed_area(&mut self,
        start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission
    ){
        self.push(MapArea::new(
            start_va,
            end_va,
            MapType::Framed,
            permission,
        ), None);
    }

    pub fn new_kernel()->Self;

    pub fn from_elf(elf_data:&[u8]) -> (Self,usize,usize);

}

// --------------------------创建内核地址空间-----------------------
extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

impl MemorySet{
    pub fn new_kernel() -> Self{
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        println!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        println!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        println!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        println!(".bss [{:#x}, {:#x})", sbss_with_stack as usize, ebss as usize);
        println!("mapping .text section");

        memory_set.push(MapArea::new(
            (stext as usize).into(),
            (etext as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::X,
        ), None);
        println!("mapping .rodata section");
        memory_set.push(MapArea::new(
            (srodata as usize).into(),
            (erodata as usize).into(),
            MapType::Identical,
            MapPermission::R,
        ), None);
        println!("mapping .data section");
        memory_set.push(MapArea::new(
            (sdata as usize).into(),
            (edata as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);
        println!("mapping .bss section");
        memory_set.push(MapArea::new(
            (sbss_with_stack as usize).into(),
            (ebss as usize).into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);
        println!("mapping physical memory");
        memory_set.push(MapArea::new(
            (ekernel as usize).into(),
            MEMORY_END.into(),
            MapType::Identical,
            MapPermission::R | MapPermission::W,
        ), None);
        memory_set
    }
} 