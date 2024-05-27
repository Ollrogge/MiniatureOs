use crate::{
    memory::{Address, PhysicalFrame, Size2MiB, Size4KiB, VirtualAddress},
    paging::{
        mapped_page_table::{MappedPageTable, PageTableFrameMapping, PageTableWalker},
        FrameAllocator, Mapper, MappingError, Page, PageTable, PageTableEntryFlags,
        TranslationError, Translator,
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
        VirtualAddress::new(self.offset + frame.start().as_u64())
    }
}

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
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        self.inner.map_to(frame, page, flags, frame_allocator)
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size2MiB> for OffsetPageTable<'a, P> {
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
        self.inner.map_to(frame, page, flags, frame_allocator)
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size4KiB> for OffsetPageTable<'a, P> {
    fn translate(&self, page: Page<Size4KiB>) -> Result<PhysicalFrame<Size4KiB>, TranslationError> {
        self.inner.translate(page)
    }
}

impl<'a, P: PageTableFrameMapping> Translator<Size2MiB> for OffsetPageTable<'a, P> {
    fn translate(&self, page: Page<Size2MiB>) -> Result<PhysicalFrame<Size2MiB>, TranslationError> {
        self.inner.translate(page)
    }
}
