// wraps the frame allocator
// stores the kernel page directory info
// places stuff in virtual memory

use super::{
    region::{self, AccessType, RegionTree, RegionType, VirtualMemoryRegion},
    virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    MemoryError,
};
use crate::error::KernelError;
use alloc::{string::String, vec::Vec};
use api::BootInfo;
use util::mutex::Mutex;
use x86_64::{
    memory::{
        FrameAllocator, PageRangeInclusive, PhysicalAddress, PhysicalFrame, VirtualAddress,
        VirtualRange,
    },
    paging::{
        linked_list_frame_allocator::LinkedListFrameAllocator, PageTable, PageTableEntryFlags,
    },
};

pub enum AllocationStrategy {
    AllocateNow,
}

static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());

// This structure is responsible for tracking the whole allocated memory as well
// as allocating new memory
pub struct MemoryManager {
    frame_allocator: LinkedListFrameAllocator,
    kernel_pml4t: PhysicalFrame,
    physical_memory_offset: usize,
    // Holds information about the complete virtual kernel memory space
    region_tree: RegionTree,
}

impl MemoryManager {
    pub const fn new() -> Self {
        Self {
            frame_allocator: LinkedListFrameAllocator::new(),
            kernel_pml4t: PhysicalFrame::new(),
            physical_memory_offset: 0,
            region_tree: RegionTree::new(),
        }
    }

    pub fn init(&mut self, boot_info: &'static BootInfo, memory_range: PageRangeInclusive) {
        self.frame_allocator.init(
            boot_info.memory_regions.iter().copied(),
            boot_info.physical_memory_offset,
        );

        self.physical_memory_offset = boot_info.physical_memory_offset;

        self.region_tree.init(memory_range)
    }

    pub fn kernel_pml4t(&self) -> PhysicalFrame {
        self.kernel_pml4t
    }

    unsafe fn kernel_page_table(&self) -> &'static mut PageTable {
        let virtual_base =
            VirtualAddress::new(self.kernel_pml4t.start() + self.physical_memory_offset as u64);

        let page_table_ptr: *mut PageTable = virtual_base.as_mut_ptr();
        &mut *page_table_ptr
    }

    // todo: lazily allocate and only back with frame on page fault
    pub fn allocate_kernel_region<U>(
        &mut self,
        size: usize,
        name: String,
        typ: RegionType,
        access_flags: PageTableEntryFlags,
        strategy: AllocationStrategy,
    ) -> Result<VirtualMemoryRegion<U>, KernelError>
    where
        U: VirtualMemoryObject,
    {
        let obj = MemoryBackedVirtualMemoryObject::create(size, strategy)?;

        let range = self.region_tree.try_allocate_region(
            name,
            typ,
            obj.size(),
            region::PlacingStrategy::Anywhere,
        )?;

        Ok(VirtualMemoryRegion::new(range, name, obj))
    }

    pub fn frame_allocator(&mut self) -> &mut LinkedListFrameAllocator {
        &mut self.frame_allocator
    }

    pub fn try_allocate_frames(&mut self, amt: usize) -> Result<Vec<PhysicalFrame>, MemoryError> {
        (0..amt)
            .map(|_| {
                self.frame_allocator
                    .allocate_frame()
                    .ok_or(MemoryError::OutOfPhysicalMemory)
            })
            .collect()
    }

    pub fn the() -> &'static Mutex<MemoryManager> {
        &MEMORY_MANAGER
    }
}
