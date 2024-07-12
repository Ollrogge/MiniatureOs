use core::default::Default;
use x86_64::{
    memory::{
        FrameAllocator, MemoryRegion, Page, PageRangeInclusive, PageSize, Size4KiB, VirtualAddress,
        VirtualRange,
    },
    paging::{Mapper, PageTableEntryFlags},
};

pub struct StackAllocator {
    page_range: PageRangeInclusive,
}

impl StackAllocator {
    pub fn new(page_range: PageRangeInclusive) -> Self {
        Self { page_range }
    }

    pub fn alloc_stack<A, M>(
        &mut self,
        frame_allocator: &mut A,
        page_table: &mut M,
        pages_cnt: usize,
    ) -> Option<Stack>
    where
        A: FrameAllocator<Size4KiB>,
        M: Mapper<Size4KiB>,
    {
        let mut range_iter = self.page_range.clone().iter();

        let guard_page = range_iter.next();
        let stack_start = range_iter.next();
        let stack_end = if pages_cnt == 1 {
            stack_start
        } else {
            // choose the (size_in_pages-2)th element, since index
            // starts at 0 and we already allocated the start page
            range_iter.nth(pages_cnt - 2)
        };

        match (guard_page, stack_start, stack_end) {
            (Some(_), Some(start), Some(end)) => {
                self.page_range = range_iter.into();

                for page in Page::range_inclusive(start, end).iter() {
                    let frame = frame_allocator
                        .allocate_frame()
                        .expect("Alloc stack: faile to alloc frame");

                    page_table
                        .map_to(
                            frame,
                            page,
                            PageTableEntryFlags::WRITABLE | PageTableEntryFlags::PRESENT,
                            frame_allocator,
                        )
                        .expect("map_to failed")
                        .flush();
                }

                // Page range goes from lower page address to higher. Since stacks
                // grow downwards we use end here for the stack top
                Some(Stack::new(
                    end.address(),
                    pages_cnt * Size4KiB::SIZE as usize,
                ))
            }
            _ => None,
        }
    }

    pub fn dealloc_stack(stack: Stack) {
        unimplemented!();
    }
}

#[derive(Clone, Copy)]
pub struct Stack {
    top: VirtualAddress,
    len: usize,
}

impl Stack {
    pub fn new(top: VirtualAddress, len: usize) -> Self {
        Self { top, len }
    }

    pub fn top(&self) -> VirtualAddress {
        self.top
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self {
            top: VirtualAddress::new(0),
            len: 0,
        }
    }
}
