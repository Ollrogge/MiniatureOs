use super::TlbFlusher;
use crate::{
    memory::{Address, PhysicalFrame, Size2MiB, Size4KiB, VirtualAddress},
    paging::{
        mapped_page_table::{MappedPageTable, PageTableFrameMapping, PageTableWalker},
        FrameAllocator, Mapper, MappingError, Page, PageTable, PageTableEntryFlags,
        TranslationError, Translator, UnmappingError,
    },
};
#[derive(Debug)]
pub struct PhysicalOffset {
    offset: u64,
}

impl PhysicalOffset {
    pub fn new(offset: u64) -> Self {
        Self { offset }
    }
}

unsafe impl PageTableFrameMapping for PhysicalOffset {
    fn frame_to_virtual(&self, frame: PhysicalFrame) -> VirtualAddress {
        VirtualAddress::new(self.offset + frame.address().as_u64())
    }
}

/// Pagetable that requires the complete physical memory space to be mapped at
/// some offset in the virtual address space.
/// Needed such that one can access the page tables using virtual addresses.
pub struct OffsetPageTable<'a, P: PageTableFrameMapping> {
    inner: MappedPageTable<'a, P>,
}

impl<'a, P: PageTableFrameMapping> OffsetPageTable<'a, P> {
    pub fn new(pml4t: &'a mut PageTable, mapping: P) -> Self {
        let inner = MappedPageTable::new(PageTableWalker::new(mapping), pml4t);
        Self { inner }
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size4KiB> for OffsetPageTable<'a, P> {
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
        self.inner.map_to(frame, page, flags, frame_allocator)
    }

    fn unmap(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysicalFrame<Size4KiB>, TlbFlusher<Size4KiB>), UnmappingError> {
        self.inner.unmap(page)
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size2MiB> for OffsetPageTable<'a, P> {
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
        self.inner.map_to(frame, page, flags, frame_allocator)
    }

    fn unmap(
        &mut self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysicalFrame<Size2MiB>, TlbFlusher<Size2MiB>), UnmappingError> {
        self.inner.unmap(page)
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size4KiB> for OffsetPageTable<'a, P> {
    fn translate(
        &self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysicalFrame<Size4KiB>, PageTableEntryFlags), TranslationError> {
        self.inner.translate(page)
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size2MiB> for OffsetPageTable<'a, P> {
    fn translate(
        &self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysicalFrame<Size2MiB>, PageTableEntryFlags), TranslationError> {
        self.inner.translate(page)
    }
}
