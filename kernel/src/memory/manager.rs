// wraps the frame allocator
// stores the kernel page directory info
// places stuff in virtual memory
extern crate alloc;

use super::{
    region::{self, AccessFlags, PlacingStrategy, RegionTree, RegionType, VirtualMemoryRegion},
    virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    MemoryError,
};
use crate::{
    allocator::init_heap,
    error::KernelError,
    multitasking::{process::Process, scheduler::Scheduler},
    serial_println,
};
use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use api::BootInfo;
use core::iter::zip;
use util::mutex::{Mutex, MutexGuard};
use x86_64::{
    interrupts::PageFaultErrorCode,
    memory::{
        FrameAllocator, Page, PageAlignedSize, PageRangeInclusive, PageSize, PhysicalAddress,
        PhysicalFrame, Size4KiB, VirtualAddress, VirtualRange,
    },
    paging::{
        linked_list_frame_allocator::LinkedListFrameAllocator,
        offset_page_table::{OffsetPageTable, PhysicalOffset},
        Mapper, MapperAllSizes, PageTable, PageTableEntryFlags,
    },
    register::Cr3,
};

#[derive(PartialEq, Clone, Copy)]
pub enum AllocationStrategy {
    Now,
    Lazy,
}

static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());

// This structure is responsible for tracking the whole allocated memory as well
// as allocating new memory
pub struct MemoryManager {
    frame_allocator: LinkedListFrameAllocator,
    kernel_pml4t: PhysicalFrame,
    kernel_page_table: Option<OffsetPageTable<'static, PhysicalOffset>>,
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
            kernel_page_table: None,
            region_tree: RegionTree::new(),
        }
    }

    // TODO: properly do error management
    pub fn init(&mut self, boot_info: &'static BootInfo) -> Result<(), KernelError> {
        self.frame_allocator.init(
            boot_info.memory_regions.iter().copied(),
            boot_info.physical_memory_offset,
        );
        self.physical_memory_offset = boot_info.physical_memory_offset;

        let (pml4t, _) = Cr3::read();
        unsafe { self.init_kernel_page_table(pml4t, boot_info.physical_memory_offset) };

        let heap_range = init_heap(
            self.kernel_page_table.as_mut().unwrap(),
            &mut self.frame_allocator,
        )?;
        self.add_memory_region(RegionType::Heap, heap_range);

        // 0xffffffff00000000 - 0xffffffff80000000
        self.add_memory_region(
            RegionType::Stack,
            PageRangeInclusive::new(
                boot_info.kernel_stack.start_page,
                Page::containing_address(boot_info.kernel.start_address() - 1u64),
            ),
        );

        self.add_memory_region(RegionType::Stack, boot_info.kernel_stack);
        self.add_memory_region(RegionType::Elf, boot_info.kernel);

        Ok(())
    }

    pub fn kernel_pml4t(&self) -> PhysicalFrame {
        self.kernel_pml4t
    }

    fn add_memory_region(&mut self, typ: RegionType, range: PageRangeInclusive) {
        self.region_tree.add_region(typ, range);
    }

    pub fn kernel_page_table(&mut self) -> &OffsetPageTable<PhysicalOffset> {
        self.kernel_page_table.as_mut().unwrap()
    }

    pub fn region_tree(&mut self) -> &mut RegionTree {
        &mut self.region_tree
    }

    unsafe fn init_kernel_page_table(
        &mut self,
        pml4t: PhysicalFrame,
        physical_memory_offset: usize,
    ) {
        self.kernel_pml4t = pml4t;
        let virtual_base = VirtualAddress::new(pml4t.start() + self.physical_memory_offset as u64);

        let page_table_ptr: *mut PageTable = virtual_base.as_mut_ptr();
        let raw_page_table = &mut *page_table_ptr;

        self.kernel_page_table = Some(OffsetPageTable::new(
            raw_page_table,
            PhysicalOffset::new(physical_memory_offset),
        ));
    }

    /*
    pub fn try_allocate_kernel_region_with_frames<U>(
        &mut self,
        frames: Vec<PhysicalFrame>,
        name: String,
        typ: RegionType,
        access_flags: PageTableEntryFlags,
    )
    */

    // todo: lazily allocate and only back with frame on page fault
    pub fn allocate_region_with_size(
        &mut self,
        process: Arc<Mutex<Process>>,
        size: PageAlignedSize,
        name: String,
        typ: RegionType,
        mut access_flags: PageTableEntryFlags,
        strategy: AllocationStrategy,
    ) -> Result<VirtualMemoryRegion, KernelError> {
        let obj = match strategy {
            AllocationStrategy::Now => {
                let frames = self.try_allocate_frames(size.in_frames())?;
                MemoryBackedVirtualMemoryObject::create(Some(frames))?
            }
            AllocationStrategy::Lazy => MemoryBackedVirtualMemoryObject::create(None)?,
        };

        let has_guard_page = typ == RegionType::Stack;

        // all kernel stacks have a guard page
        let region_size = match has_guard_page {
            true => size + Size4KiB::SIZE,
            _ => size,
        };

        let page_range: PageRangeInclusive = self.region_tree.try_allocate_size_in_region(
            name.clone(),
            typ,
            region_size,
            region::PlacingStrategy::Anywhere,
        )?;

        // all kernel stacks have a guard page
        if has_guard_page {
            self.kernel_page_table
                .as_mut()
                .unwrap()
                .map_to(
                    PhysicalFrame::containing_address(PhysicalAddress::new(0)),
                    page_range.start_page,
                    PageTableEntryFlags::NONE,
                    &mut self.frame_allocator,
                )?
                .ignore();
        }

        if strategy == AllocationStrategy::Now {
            assert_eq!(page_range.len() - 1, obj.frames().len());
            // skip guard page, index 0 since stack grows downwards => lowest address is end of stack
            for (frame, page) in zip(obj.frames(), page_range.iter().skip(1)) {
                unsafe {
                    process
                        .lock()
                        .address_space()
                        .map_to(frame.clone(), page, access_flags)?
                        .flush();
                }
            }
        } else {
            access_flags.remove(PageTableEntryFlags::PRESENT);
            for page in page_range.iter().skip(1) {
                unsafe {
                    process
                        .lock()
                        .address_space()
                        .map_to(
                            PhysicalFrame::containing_address(PhysicalAddress::new(0)),
                            page,
                            access_flags,
                        )?
                        .flush();
                }
            }
        }

        Ok(VirtualMemoryRegion::new(
            page_range,
            name,
            Box::new(obj),
            typ,
            access_flags.into(),
        ))
    }

    pub fn deallocate_frames(&mut self, frames: &Vec<PhysicalFrame>) {
        for frame in frames {
            self.frame_allocator.deallocate_frame(*frame);
        }
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

    pub fn frame_allocator(&mut self) -> &mut LinkedListFrameAllocator {
        &mut self.frame_allocator
    }

    pub fn the() -> &'static Mutex<MemoryManager> {
        &MEMORY_MANAGER
    }
}

pub struct FrameAllocatorDelegate;
unsafe impl FrameAllocator<Size4KiB> for FrameAllocatorDelegate {
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        MemoryManager::the()
            .lock()
            .frame_allocator()
            .allocate_frame()
    }

    fn deallocate_frame(&mut self, frame: PhysicalFrame<Size4KiB>) {
        MemoryManager::the()
            .lock()
            .frame_allocator()
            .deallocate_frame(frame)
    }
}
