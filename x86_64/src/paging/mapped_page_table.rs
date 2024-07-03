use crate::{
    memory::{
        Address, FrameAllocator, Page, PageSize, PhysicalFrame, Size2MiB, Size4KiB, VirtualAddress,
    },
    paging::{
        Mapper, MappingError, PageTable, PageTableEntry, PageTableEntryFlags, TlbFlusher,
        TranslationError, Translator, UnmappingError,
    },
};
use core::ops::Add;
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
            pagetable_entry.set_address(frame.address(), flags);

            let virtual_address = self.page_table_frame_mapping.frame_to_virtual(frame);

            let table = unsafe { PageTable::initialize_empty_at_address(virtual_address) };
            table
        } else {
            if !flags.is_empty() && !pagetable_entry.flags().contains(flags) {
                pagetable_entry.add_flags(flags);
            }

            let virtual_address = self
                .page_table_frame_mapping
                .frame_to_virtual(pagetable_entry.physical_frame());

            unsafe { PageTable::at_address(virtual_address) }
        };

        Some(table)
    }

    pub fn get_pagetable<'a>(
        &self,
        pagetable_entry: &'a PageTableEntry,
    ) -> Option<&'a mut PageTable> {
        match pagetable_entry.is_unused() {
            true => None,
            false => {
                let virtual_address = self
                    .page_table_frame_mapping
                    .frame_to_virtual(pagetable_entry.physical_frame());

                unsafe { Some(PageTable::at_address(virtual_address)) }
            }
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
    ) -> Result<TlbFlusher<Size4KiB>, MappingError>
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
            pte.set_address(frame.address(), flags);
            Ok(TlbFlusher::new(page))
        }
    }

    fn unmap(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysicalFrame<Size4KiB>, TlbFlusher<Size4KiB>), UnmappingError> {
        let l4 = &mut self.pml4t;
        let l3 = self
            .walker
            .get_pagetable(&mut l4[page.address.l4_index()])
            .unwrap();
        let l2 = self
            .walker
            .get_pagetable(&mut l3[page.address.l3_index()])
            .unwrap();

        let l1 = self
            .walker
            .get_pagetable(&mut l2[page.address.l2_index()])
            .unwrap();

        let pte = &mut l1[page.address().l1_index()];

        if !pte.flags().contains(PageTableEntryFlags::PRESENT) {
            return Err(UnmappingError::PageNotMapped);
        }

        pte.set_unused();

        Ok((
            PhysicalFrame::containing_address(pte.address()),
            TlbFlusher::new(page),
        ))
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size2MiB> for MappedPageTable<'a, P> {
    fn map_to<A>(
        &mut self,
        frame: PhysicalFrame<Size2MiB>,
        page: Page<Size2MiB>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<TlbFlusher<Size2MiB>, MappingError>
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

        let pte = &mut l2[page.address.l2_index()];

        if pte.is_present() {
            Err(MappingError::PageAlreadyMapped)
        } else {
            pte.set_address(frame.address(), flags | PageTableEntryFlags::HUGE_PAGE);
            Ok(TlbFlusher::new(page))
        }
    }

    fn unmap(
        &mut self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysicalFrame<Size2MiB>, TlbFlusher<Size2MiB>), UnmappingError> {
        let l4 = &mut self.pml4t;
        let l3 = self
            .walker
            .get_pagetable(&mut l4[page.address.l4_index()])
            .unwrap();
        let l2 = self
            .walker
            .get_pagetable(&mut l3[page.address.l3_index()])
            .unwrap();

        let pte = &mut l2[page.address.l2_index()];

        if !pte.flags().contains(PageTableEntryFlags::PRESENT) {
            return Err(UnmappingError::PageNotMapped);
        }

        pte.set_unused();

        Ok((
            PhysicalFrame::containing_address(pte.address()),
            TlbFlusher::new(page),
        ))
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size4KiB> for MappedPageTable<'a, P> {
    fn translate(
        &self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysicalFrame<Size4KiB>, PageTableEntryFlags), TranslationError> {
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
            Ok((
                PhysicalFrame::containing_address(pte.address()),
                pte.flags(),
            ))
        } else {
            Err(TranslationError::NotMapped)
        }
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size2MiB> for MappedPageTable<'a, P> {
    fn translate(
        &self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysicalFrame<Size2MiB>, PageTableEntryFlags), TranslationError> {
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
            Ok((
                PhysicalFrame::containing_address(pte.address()),
                pte.flags(),
            ))
        } else {
            Err(TranslationError::NotMapped)
        }
    }
}
