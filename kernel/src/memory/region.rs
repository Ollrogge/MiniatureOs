use super::{virtual_memory_object::VirtualMemoryObject, MemoryError};
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use util::range_allocator::{self, RangeAllocator};
use x86_64::memory::{
    Address, Page, PageAlignedSize, PageRangeInclusive, Region, Size4KiB, VirtualAddress,
    VirtualRange,
};

pub enum AccessType {
    Read,
    ReadWrite,
}

#[derive(PartialEq, Clone, Copy)]
pub enum RegionType {
    Stack,
    Heap,
    Elf,
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

    pub fn try_allocate_range_in_region(
        &mut self,
        name: String,
        typ: RegionType,
        range: PageRangeInclusive,
    ) -> Result<(), MemoryError> {
        if let Some(region_info) = self.regions.iter_mut().find(|r| r.typ == typ) {
            let res = region_info
                .allocator
                .try_allocate_range(range.clone().into());
            if !res {
                Err(MemoryError::InvalidRange)
            } else {
                region_info.subregions.insert(name, range);
                Ok(())
            }
        } else {
            Err(MemoryError::Other)
        }
    }

    pub fn try_allocate_size_in_region(
        &mut self,
        name: String,
        typ: RegionType,
        size: PageAlignedSize,
        _: PlacingStrategy,
    ) -> Result<PageRangeInclusive, MemoryError> {
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
                region_info.subregions.insert(name, range.clone());
            }

            res
        } else {
            Err(MemoryError::Other)
        }
    }

    pub fn try_deallocate_from_region(
        &mut self,
        name: String,
        typ: RegionType,
    ) -> Result<(), MemoryError> {
        if let Some(region_info) = self.regions.iter_mut().find(|r| r.typ == typ) {
            if let Some(range) = region_info.subregions.remove(&name) {
                region_info.allocator.deallocate_range(range.into());
                return Ok(());
            }
        }
        Err(MemoryError::InvalidRegion)
    }
}

//Region with a base address, size and permissions
// Only knows to which VMObject it refers to, no physical frame
pub struct VirtualMemoryRegion<U: VirtualMemoryObject> {
    range: PageRangeInclusive,
    name: String,
    obj: U,
    contains_guard_page: bool,
}

impl<U: VirtualMemoryObject> VirtualMemoryRegion<U> {
    pub fn new(range: PageRangeInclusive, name: String, obj: U, contains_guard_page: bool) -> Self {
        Self {
            range,
            name,
            obj,
            contains_guard_page,
        }
    }

    pub fn start(&self) -> VirtualAddress {
        if self.contains_guard_page {
            (self.range.start_page + 1u64).start_address()
        } else {
            self.range.start_page.start_address()
        }
    }

    pub fn end(&self) -> VirtualAddress {
        self.range.end_page.end_address()
    }

    pub fn size(&self) -> usize {
        if self.contains_guard_page {
            self.range.size() - self.range.start_page().size()
        } else {
            self.range.size()
        }
    }
}
