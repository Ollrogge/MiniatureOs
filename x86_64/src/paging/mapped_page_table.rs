use crate::{
    memory::{Address, Page, PageSize, PhysicalFrame, Size2MiB, Size4KiB, VirtualAddress},
    paging::{
        FrameAllocator, Mapper, MappingError, PageTable, PageTableEntry, PageTableEntryFlags,
        TranslationError, Translator,
    },
};
/// Provides a virtual address mapping for physical page table frames.
///
/// This only works if the physical address space is somehow mapped to the virtual
/// address space, e.g. at an offset.
///
/// ## Safety
///
/// This trait is unsafe to implement because the implementer must ensure that
/// `frame_to_pointer` returns a valid page table pointer for any given physical frame.
pub unsafe trait PageTableFrameMapping {
    /// Translate the given physical frame to a virtual page table pointer.
    fn frame_to_virtual(&self, frame: PhysicalFrame) -> VirtualAddress;
}

pub struct MappedPageTable<'a, P: PageTableFrameMapping> {
    walker: PageTableWalker<P>,
    pml4t: &'a mut PageTable,
}

impl<'a, P: PageTableFrameMapping> MappedPageTable<'a, P> {
    pub fn new(walker: PageTableWalker<P>, pml4t: &'a mut PageTable) -> Self {
        Self { walker, pml4t }
    }
}

/// This struct only exists to avoid borrowing self twice in the map_to func
pub struct PageTableWalker<P: PageTableFrameMapping> {
    page_table_frame_mapping: P,
}

impl<P: PageTableFrameMapping> PageTableWalker<P> {
    pub fn new(mapping: P) -> Self {
        Self {
            page_table_frame_mapping: mapping,
        }
    }
    /// Allocates pagetable or returns it if already existing
    // assumes 1:1 mapping for physical frames holding pagetable data
    // using virtual addresses here because we are in long mode and all memory accesses
    // are based on virtual memory. So just to make it explicit
    pub fn get_or_allocate_pagetable<'a, A>(
        &self,
        pagetable_entry: &'a mut PageTableEntry,
        flags: PageTableEntryFlags,
        allocator: &mut A,
    ) -> Option<&'a mut PageTable>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let table = if pagetable_entry.is_unused() {
            let frame = allocator.allocate_frame()?;
            pagetable_entry.set_address(frame.start(), flags);

            let virtual_address = self.page_table_frame_mapping.frame_to_virtual(frame);

            let table = PageTable::initialize_empty_at_address(virtual_address);
            table
        } else {
            if !flags.is_empty() && !pagetable_entry.flags().contains(flags) {
                pagetable_entry.set_flags(pagetable_entry.flags() | flags);
            }

            let virtual_address = self
                .page_table_frame_mapping
                .frame_to_virtual(pagetable_entry.physical_frame());

            PageTable::at_address(virtual_address)
        };

        Some(table)
    }

    pub fn get_pagetable<'a>(&self, pagetable_entry: &'a PageTableEntry) -> Option<&'a PageTable> {
        if !pagetable_entry.is_unused() {
            let virtual_address =
                VirtualAddress::new(pagetable_entry.physical_frame().start().as_u64());
            Some(PageTable::at_address(virtual_address))
        } else {
            None
        }
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size4KiB> for MappedPageTable<'a, P> {
    fn map_to<A>(
        &mut self,
        frame: PhysicalFrame<Size4KiB>,
        page: Page<Size4KiB>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let parent_flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::USER_ACCESSIBLE;
        let l4 = &mut self.pml4t;
        let l3 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l4[page.address.l4_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l2 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l3[page.address.l3_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l1 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l2[page.address.l2_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;

        let pte = &mut l1[page.address.l1_index()];

        if pte.is_present() {
            Err(MappingError::PageAlreadyMapped)
        } else {
            pte.set_address(frame.start(), flags);
            Ok(())
        }
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size2MiB> for MappedPageTable<'a, P> {
    fn map_to<A>(
        &mut self,
        frame: PhysicalFrame<Size2MiB>,
        page: Page<Size2MiB>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let parent_flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::USER_ACCESSIBLE
            | PageTableEntryFlags::HUGE_PAGE;
        let l4 = &mut self.pml4t;
        let l3 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l4[page.address.l4_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l2 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l3[page.address.l3_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;

        let pte = &mut l2[page.address.l2_index()];

        if pte.is_present() {
            Err(MappingError::PageAlreadyMapped)
        } else {
            pte.set_address(frame.start(), flags);
            Ok(())
        }
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size4KiB> for MappedPageTable<'a, P> {
    fn translate(&self, page: Page<Size4KiB>) -> Result<PhysicalFrame<Size4KiB>, TranslationError> {
        let l4 = &self.pml4t;
        let l3 = self
            .walker
            .get_pagetable(&l4[page.address.l4_index()])
            .ok_or(TranslationError::NotMapped)?;
        let l2 = self
            .walker
            .get_pagetable(&l3[page.address.l3_index()])
            .ok_or(TranslationError::NotMapped)?;
        let l1 = self
            .walker
            .get_pagetable(&l2[page.address.l2_index()])
            .ok_or(TranslationError::NotMapped)?;

        let pte = &l1[page.address.l1_index()];

        if pte.is_present() {
            Ok(PhysicalFrame::containing_address(pte.address()))
        } else {
            Err(TranslationError::NotMapped)
        }
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size2MiB> for MappedPageTable<'a, P> {
    fn translate(&self, page: Page<Size2MiB>) -> Result<PhysicalFrame<Size2MiB>, TranslationError> {
        let l4 = &self.pml4t;
        let l3 = self
            .walker
            .get_pagetable(&l4[page.address.l4_index()])
            .ok_or(TranslationError::NotMapped)?;
        let l2 = self
            .walker
            .get_pagetable(&l3[page.address.l3_index()])
            .ok_or(TranslationError::NotMapped)?;

        let pte = &l2[page.address.l2_index()];

        if pte.is_present() {
            Ok(PhysicalFrame::containing_address(pte.address()))
        } else {
            Err(TranslationError::NotMapped)
        }
    }
}
