use crate::{
    memory::{
        Address, FrameAllocator, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame,
        Size4KiB, VirtualAddress,
    },
    serial_println,
};
use util::intrusive_linked_list::{IntrusiveLinkedList, ListNode};

// This frame allocator assumes that the complete physical memory space is mapped
// at an offset in the virtual memory space
pub struct LinkedListFrameAllocator {
    free_list: IntrusiveLinkedList,
    // just out of interest
    free_list_size: usize,
    offset: usize,
}

impl LinkedListFrameAllocator {
    pub const fn new() -> Self {
        Self {
            free_list: IntrusiveLinkedList::new(),
            free_list_size: 0,
            offset: 0,
        }
    }

    pub fn init<I, D>(&mut self, map: I, offset: usize)
    where
        I: Iterator<Item = D>,
        D: MemoryRegion,
    {
        let mut sz = 0x0;
        map.filter(|region| region.is_usable()).for_each(|region| {
            let start = region.start();
            let end = region.end();

            assert!(Size4KiB::is_aligned(start as usize));

            // Map physical address to virtual one such that we can access frame
            // without getting a pagefault
            let start_page: Page<Size4KiB> =
                Page::containing_address(VirtualAddress::new(start + offset as u64));
            let end_page =
                Page::containing_address(VirtualAddress::new(end - 1u64 + offset as u64));

            Page::range_inclusive(start_page, end_page).for_each(|page| {
                //serial_println!("Pushing to list: {:#x}", frame.start());
                let node = unsafe {
                    ListNode::new_at_address(
                        usize::try_from(page.start_address().as_u64()).unwrap(),
                    )
                };

                self.free_list.push_front(node);
                sz += 1;
            });
        });

        self.free_list_size = sz;
        self.offset = offset;

        serial_println!("Frame allocator total frames: {}", sz);
    }
}

unsafe impl FrameAllocator<Size4KiB> for LinkedListFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        if let Some(node) = self.free_list.pop_front() {
            self.free_list_size -= 1;

            Some(PhysicalFrame::containing_address(PhysicalAddress::new(
                (node.address() - self.offset) as u64,
            )))
        } else {
            None
        }
    }

    fn deallocate_frame(&mut self, frame: PhysicalFrame<Size4KiB>) {
        let node = unsafe {
            ListNode::new_at_address(usize::try_from(frame.start()).unwrap() + self.offset)
        };

        self.free_list.push_front(node);
        self.free_list_size += 1;
    }
}
