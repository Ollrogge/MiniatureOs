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
