use super::{
    manager::MemoryManager,
    virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    MemoryError,
};
use crate::{
    error::KernelError, memory::manager::FrameAllocatorDelegate, multitasking::process::Process,
    serial_println,
};
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::{borrow::Borrow, default, ops::Drop};
use util::range_allocator::{self, RangeAllocator};
use x86_64::{
    memory::{
        Address, Page, PageAlignedSize, PageRangeInclusive, PhysicalAddress, PhysicalFrame, Region,
        Size4KiB, VirtualAddress,
    },
    paging::PageTableEntryFlags,
};

pub enum AccessType {
    Read,
    ReadWrite,
}

#[derive(PartialEq, Clone, Copy, Debug, Default)]
pub enum RegionType {
    Stack,
    Heap,
    Elf,
    #[default]
    Other,
}

pub enum PlacingStrategy {
    Anywhere,
}

pub struct RegionInfo {
    pub allocator: RangeAllocator,
    pub subregions: BTreeMap<String, PageRangeInclusive>,
    pub typ: RegionType,
}

impl RegionInfo {
    pub fn new(typ: RegionType, range: PageRangeInclusive) -> Self {
        Self {
            allocator: RangeAllocator::new(range.into()),
            subregions: BTreeMap::new(),
            typ,
        }
    }
}

pub struct RegionTree {
    regions: Vec<RegionInfo>,
}

impl RegionTree {
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn add_region(&mut self, typ: RegionType, range: PageRangeInclusive) {
        let info = RegionInfo::new(typ, range);
        self.regions.push(info);
    }

    pub fn try_allocate_range_in_region<N>(
        &mut self,
        name: N,
        typ: RegionType,
        range: PageRangeInclusive,
    ) -> Result<(), MemoryError>
    where
        N: Into<String>,
    {
        if let Some(region_info) = self.regions.iter_mut().find(|r| r.typ == typ) {
            let res = region_info
                .allocator
                .try_allocate_range(range.clone().into());
            if !res {
                Err(MemoryError::InvalidRange)
            } else {
                region_info.subregions.insert(name.into(), range);
                Ok(())
            }
        } else {
            Err(MemoryError::Other)
        }
    }

    pub fn try_allocate_size_in_region<N>(
        &mut self,
        name: N,
        typ: RegionType,
        size: PageAlignedSize,
        _: PlacingStrategy,
    ) -> Result<PageRangeInclusive, MemoryError>
    where
        N: Into<String>,
    {
        if size.in_bytes() == 0 {
            return Err(MemoryError::InvalidSize);
        }

        if let Some(region_info) = self.regions.iter_mut().find(|r| r.typ == typ) {
            let res = region_info
                .allocator
                .try_allocate_size(size.in_bytes())
                .map(|x| {
                    PageRangeInclusive::new(
                        Page::<Size4KiB>::containing_address(VirtualAddress::new(x.start)),
                        Page::<Size4KiB>::containing_address(VirtualAddress::new(x.end - 1)),
                    )
                })
                .ok_or(MemoryError::OutOfVirtualMemory);

            if let Ok(range) = &res {
                region_info.subregions.insert(name.into(), range.clone());
            }

            res
        } else {
            Err(MemoryError::Other)
        }
    }

    pub fn try_deallocate_from_region<N>(
        &mut self,
        typ: RegionType,
        name: N,
    ) -> Result<PageRangeInclusive, MemoryError>
    where
        N: Borrow<String>,
    {
        if let Some(region_info) = self.regions.iter_mut().find(|r| r.typ == typ) {
            if let Some(range) = region_info.subregions.remove(name.borrow()) {
                region_info.allocator.deallocate_range(range.into());
                return Ok(range);
            }
        }
        Err(MemoryError::InvalidRegion)
    }
}

//Region with a base address, size and permissions
// Only knows to which VMObject it refers to, no physical frame
#[derive(Default)]
pub struct VirtualMemoryRegion<U: VirtualMemoryObject> {
    range: PageRangeInclusive,
    name: String,
    obj: U,
    typ: RegionType,
}

impl<U: VirtualMemoryObject> VirtualMemoryRegion<U> {
    pub fn new<N>(range: PageRangeInclusive, name: N, obj: U, typ: RegionType) -> Self
    where
        N: Into<String>,
    {
        Self {
            range,
            name: name.into(),
            obj,
            typ,
        }
    }

    pub fn start(&self) -> VirtualAddress {
        if self.typ == RegionType::Stack {
            (self.range.start_page + 1u64).start_address()
        } else {
            self.range.start_page.start_address()
        }
    }

    pub fn end(&self) -> VirtualAddress {
        self.range.end_page.end_address()
    }

    pub fn size(&self) -> usize {
        if self.typ == RegionType::Stack {
            self.range.size() - self.range.start_page().size()
        } else {
            self.range.size()
        }
    }

    pub fn typ(&self) -> RegionType {
        self.typ
    }
}

impl<U: VirtualMemoryObject> Drop for VirtualMemoryRegion<U> {
    fn drop(&mut self) {
        serial_println!("Drop VirtualMemoryRegion: {:?} {:?}", self.typ, self.name);
        MemoryManager::the()
            .lock()
            .region_tree()
            .try_deallocate_from_region(self.typ, &self.name)
            .unwrap();

        let process = Process::current();
        let mut process_guard = process.lock();
        let address_space = process_guard.address_space();

        for page in self.range.iter() {
            // not all pages might be mapped so ignore errors
            if let Ok((_, flusher)) = address_space.unmap(page) {
                flusher.flush();
            }
        }
    }
}
