use crate::{
    memory::{
        FrameAllocator, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB,
        VirtualAddress,
    },
    serial_println,
};
use util::intrusive_linked_list::{IntrusiveLinkedList, ListNode};

// This frame allocator assumes that the complete physical memory space is mapped
// at an offset in the virtual memory space
pub struct LinkedListFrameAllocator {
    free_list: IntrusiveLinkedList,
    offset: usize,
}

impl LinkedListFrameAllocator {
    pub fn new<I, D>(map: I, offset: usize) -> Self
    where
        I: Iterator<Item = D>,
        D: MemoryRegion,
    {
        let mut list = IntrusiveLinkedList::new();
        for region in map {
            if !region.is_usable() {
                continue;
            }

            let start = region.start();
            let end = region.end();

            assert!(Size4KiB::is_aligned(start as usize));

            // Map physical address to virtual one such that we can access them
            // without getting a pagefault
            let start_page: Page<Size4KiB> =
                Page::containing_address(VirtualAddress::new(start + offset as u64));
            let end_page = Page::containing_address(VirtualAddress::new(end + offset as u64));

            for frame in Page::range_inclusive(start_page, end_page) {
                //serial_println!("Pushing to list: {:#x}", frame.start());

                let node =
                    unsafe { ListNode::new_at_address(usize::try_from(frame.start()).unwrap()) };

                list.push_front(node);
            }
        }
        Self {
            free_list: list,
            offset,
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for LinkedListFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        self.free_list.pop_front().map(|node| {
            PhysicalFrame::containing_address(PhysicalAddress::new(
                (node.address() - self.offset) as u64,
            ))
        })
    }

    fn deallocate_frame(&mut self, frame: PhysicalFrame<Size4KiB>) {
        let node = unsafe {
            ListNode::new_at_address(usize::try_from(frame.start()).unwrap() + self.offset)
        };

        self.free_list.push_front(node);
    }
}
