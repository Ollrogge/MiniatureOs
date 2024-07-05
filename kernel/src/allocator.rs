use core::ops::Add;
use util::mutex::{Mutex, MutexGuard};
/*
use bump allocator for frame allocations for now. Only handle frame deallocations later
*/
use x86_64::{
    memory::{FrameAllocator, Page, Size4KiB, VirtualAddress},
    paging::{Mapper, PageTableEntryFlags},
};

pub mod buddy_allocator;
pub mod stack_allocator;
use buddy_allocator::BuddyAllocator;

pub const HEAP_START: VirtualAddress = VirtualAddress::new(0x_4444_4444_0000);
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
pub static ALLOCATOR: Locked<BuddyAllocator> = Locked::new(BuddyAllocator::new());

pub fn init_heap<M, A>(page_table: &mut M, frame_allocator: &mut A)
where
    M: Mapper<Size4KiB>,
    A: FrameAllocator<Size4KiB>,
{
    // map heap range
    let start_page = Page::containing_address(HEAP_START);
    let end_page = Page::containing_address(HEAP_START + HEAP_SIZE - 1usize);
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate frame for kernel heap");

        let flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::NO_EXECUTE;

        page_table
            .map_to(frame, page, flags, frame_allocator)
            .expect("Failed to map heap page")
            .flush();
    }

    let guard_page = Page::containing_address(HEAP_START + HEAP_SIZE);
    let frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate frame for guard page");

    page_table
        .map_to(
            frame,
            guard_page,
            PageTableEntryFlags::NONE,
            frame_allocator,
        )
        .expect("Failed to map guard page")
        .flush();

    let mut allocator = ALLOCATOR.lock();
    allocator.init(HEAP_START, HEAP_SIZE);
}

pub struct Locked<A> {
    inner: Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> MutexGuard<A> {
        self.inner.lock()
    }
}

/*
pub mod buddy_frame_allocator;

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

struct DummyAllocator;

unsafe impl GlobalAlloc for DummyAllocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut() // Simulates an allocator by returning a null pointer
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Memory is not actually freed
    }
}

#[global_allocator]
static ALLOCATOR: DummyAllocator = DummyAllocator;
*/
