use crate::memory::manager::FrameAllocatorDelegate;
use x86_64::{
    memory::{Page, PhysicalAddress, PhysicalFrame, Size4KiB, VirtualAddress},
    paging::{
        offset_page_table::{OffsetPageTable, PhysicalOffset},
        Mapper, MappingError, PageTable, PageTableEntryFlags, TlbFlusher, UnmappingError,
    },
    register::{Cr3, Cr3Flags},
};

pub struct AddressSpace {
    cr3: u64,
    page_table: OffsetPageTable<'static, PhysicalOffset>,
}

impl AddressSpace {
    pub fn new(cr3: u64, phyical_memory_offset: usize) -> Self {
        let virtual_base = VirtualAddress::new(cr3 + phyical_memory_offset as u64);

        let page_table_ptr: *mut PageTable = virtual_base.as_mut_ptr();
        let raw_page_table = unsafe { &mut *page_table_ptr };

        let page_table =
            OffsetPageTable::new(raw_page_table, PhysicalOffset::new(phyical_memory_offset));

        Self { cr3, page_table }
    }

    pub unsafe fn map_to(
        &mut self,
        frame: PhysicalFrame,
        page: Page,
        flags: PageTableEntryFlags,
    ) -> Result<TlbFlusher<Size4KiB>, MappingError> {
        self.page_table
            .map_to(frame, page, flags, &mut FrameAllocatorDelegate)
    }

    pub fn unmap(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysicalFrame<Size4KiB>, TlbFlusher<Size4KiB>), UnmappingError> {
        self.page_table.unmap(page)
    }

    pub fn cr3(&self) -> u64 {
        self.cr3
    }
}
