use crate::{
    memory::{
        Address, FrameAllocator, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame,
        Size4KiB, VirtualAddress,
    },
    serial_println,
};
use core::{
    pin::Pin,
    ptr,
    ptr::{addr_of_mut, NonNull},
};
use util::intrusive_linked_list::{BoxAt, IntrusiveLinkedList, Linked, Links};

struct Node {
    links: Links<Node>,
}

impl Node {
    pub const fn new() -> Self {
        Self {
            links: Links::new(),
        }
    }

    pub fn address(&self) -> usize {
        self as *const Node as usize
    }
}

unsafe impl Linked<Links<Node>> for Node {
    type Handle = Pin<BoxAt<Self>>;

    fn into_ptr(handle: Self::Handle) -> NonNull<Node> {
        unsafe { NonNull::from(BoxAt::leak(Pin::into_inner_unchecked(handle))) }

        //unsafe { NonNull::new_unchecked(Pin::into_inner_unchecked(handle) as *mut Node) }
    }

    unsafe fn from_ptr(ptr: NonNull<Node>) -> Self::Handle {
        Pin::new_unchecked(BoxAt::from_raw(ptr.as_ptr()))
    }

    unsafe fn links(target: NonNull<Node>) -> NonNull<Links<Node>> {
        let links = ptr::addr_of_mut!((*target.as_ptr()).links);
        NonNull::new_unchecked(links)
    }
}

// This frame allocator assumes that the complete physical memory space is mapped
// at an offset in the virtual memory space
pub struct LinkedListFrameAllocator {
    free_list: IntrusiveLinkedList<Node>,
    // just out of interest
    free_list_size: usize,
    // offset of physical address in virtual address space
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

            Page::range_inclusive(start_page, end_page)
                .iter()
                .for_each(|page| {
                    //serial_println!("Pushing to list: {:#x}", frame.start());
                    let node = BoxAt::pin(
                        usize::try_from(page.start_address().as_u64()).unwrap(),
                        Node::new(),
                    );

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
        let node = BoxAt::pin(
            usize::try_from(frame.start()).unwrap() + self.offset,
            Node::new(),
        );

        self.free_list.push_front(node);
        self.free_list_size += 1;
    }
}
