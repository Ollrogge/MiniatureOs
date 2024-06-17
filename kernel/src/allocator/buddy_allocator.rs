//! This module implements a buddy frame allocator
//!
extern crate alloc;
use super::Locked;
use alloc::{
    alloc::{GlobalAlloc, Layout},
    borrow::ToOwned,
};
use core::{
    cmp::{max, min},
    mem::MaybeUninit,
    ptr::NonNull,
};
use x86_64::{
    memory::{
        Address, FrameAllocator, MemoryRegion, PageSize, PhysicalAddress, PhysicalFrame,
        PhysicalMemoryRegion, PhysicalMemoryRegionType, Region, Size2MiB, Size4KiB, VirtualAddress,
    },
    println,
};
// todo: make a frame_allocators directory
//  - lib (or mod idk) file contains the trait def
//  - then have 1 file for buddy 1 for Bump

// Basic problem: Buddy allocator requires holding state about free buddies.
// This however would require dynamic memory which we are trying to implement with this
// allocator.
// There are two possible solutions I can think about to this:
// 1: Save FreeBlock metadata at the beginning of the memory regions we
//  are trying to allocate (similar to e.g. glibc heap). This however would
// require a lot of use of raw pointers and just screams "heap exploitation"
// 2: Pre-allocate an array to hold a fixed number of FreeBlocks. This approach
// has an upper boundary to the amount of FreeBlocks we can track and is therefore
// not as dynamic as the first one. However it feels safer and good enough for now
//
// => Use approach 2 for now
//
// cons of buddy_frame allocator: only supports power of 2 allocations

// max order is 1 MiB => max buddy size is 512kib
const MAX_ORDER: usize = 20;

const LIST_SIZE: usize = 512;

fn previous_power_of_two(num: u64) -> u64 {
    1 << (u64::BITS - num.leading_zeros() - 1)
}

trait LinkedListTrait {
    fn pop_front(&mut self) -> Option<NonNull<Chunk>>;
    fn remove(&mut self, start: u64) -> Option<NonNull<Chunk>>;
    fn front(&self) -> Option<NonNull<Chunk>>;
    fn is_empty(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct Chunk {
    next: Option<NonNull<Chunk>>,
    size: u64,
}

unsafe impl Send for Chunk {}

impl Chunk {
    pub fn reset(&mut self) {
        self.next = None;
        self.size = 0;
    }

    pub fn new(next: Option<NonNull<Chunk>>, size: u64) -> Self {
        Self { next, size }
    }

    pub unsafe fn new_at_address(address: VirtualAddress, size: u64) -> &'static mut Chunk {
        let node: &'static mut Chunk = &mut *address.as_mut_ptr();
        node.size = size;
        node.next = None;

        node
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn start(&self) -> u64 {
        self as *const Self as u64
    }

    pub fn address(&self) -> VirtualAddress {
        VirtualAddress::new(self.start())
    }
}

// Linked list providing its own storage for the nodes
struct LinkedListWithStorage {
    // You can think of MaybeUninit<T> as being a bit like Option<T>
    // but without any of the run-time tracking and without any of the safety checks.
    pub nodes: [MaybeUninit<Chunk>; LIST_SIZE],
    pub head: Option<NonNull<Chunk>>,
}

impl LinkedListWithStorage {
    pub fn new() -> Self {
        const UNINIT: MaybeUninit<Chunk> = MaybeUninit::uninit();

        let mut list = Self {
            nodes: [UNINIT; LIST_SIZE],
            head: None,
        };

        for node in &mut list.nodes {
            let node_ptr: *mut Chunk = node.as_mut_ptr();
            let node_ref = unsafe { &mut *node_ptr };

            node_ref.next = list.head;
            list.head = Some(NonNull::new(node_ptr).unwrap());
        }

        list
    }

    fn push_front(&mut self, block: *mut Chunk) {
        unsafe {
            (*block).next = self.head;
        }
        self.head = Some(NonNull::new(block).unwrap());
    }
}

impl LinkedListTrait for LinkedListWithStorage {
    fn pop_front(&mut self) -> Option<NonNull<Chunk>> {
        if let Some(mut block) = self.head.take() {
            self.head = unsafe { block.as_mut().next.take() };
            Some(block)
        } else {
            None
        }
    }

    fn remove(&mut self, _: u64) -> Option<NonNull<Chunk>> {
        None
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn front(&self) -> Option<NonNull<Chunk>> {
        self.head.as_ref().map(|non_null| non_null.clone())
    }
}

#[derive(Clone, Copy)]
struct LinkedList {
    // You can think of MaybeUninit<T> as being a bit like Option<T>
    // but without any of the run-time tracking and without any of the safety checks.
    pub head: Option<NonNull<Chunk>>,
}

unsafe impl Send for LinkedList {}

impl LinkedList {
    pub const fn new() -> Self {
        Self { head: None }
    }

    /// Add a node to the list.
    /// O(1) runtime
    fn push_front(&mut self, block: &mut Chunk) {
        block.next = self.head;
        self.head = Some(NonNull::new(block).unwrap());
    }
}

impl LinkedListTrait for LinkedList {
    fn pop_front(&mut self) -> Option<NonNull<Chunk>> {
        if let Some(mut block) = self.head.take() {
            self.head = unsafe { block.as_mut().next.take() };
            Some(block)
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn front(&self) -> Option<NonNull<Chunk>> {
        self.head.as_ref().map(|non_null| non_null.clone())
    }
    /// Remove node starting at start from list.
    /// takes O(n) time
    fn remove(&mut self, start: u64) -> Option<NonNull<Chunk>> {
        let mut last_chunk: Option<NonNull<Chunk>> = None;
        let mut cur_chunk = self.front();

        while let Some(mut node_ptr) = cur_chunk {
            let node = unsafe { node_ptr.as_mut() };
            if node.start() == start {
                // If the node to be removed is found, update the links
                match last_chunk {
                    Some(mut last_node_ptr) => unsafe { last_node_ptr.as_mut().next = node.next },
                    None => self.head = node.next,
                }

                // Return the mutable reference to the removed node
                return Some(node_ptr);
            }

            // Move to the next node
            last_chunk = cur_chunk;
            cur_chunk = node.next;
        }

        None
    }
}

pub struct BuddyAllocator {
    buddies: [LinkedList; MAX_ORDER],
}

impl<'a> BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            buddies: [LinkedList::new(); MAX_ORDER],
        }
    }

    pub fn init_from_memory_map<I>(&mut self, memory_map: I)
    where
        I: Iterator<Item = PhysicalMemoryRegion>,
    {
        for region in memory_map {
            if !region.is_usable() {
                continue;
            }

            let start = VirtualAddress::new(region.start());
            let end = VirtualAddress::new(region.end() - 1);

            self.add_region(start, end);
        }
    }

    pub fn init(&mut self, start: VirtualAddress, size: usize) {
        self.add_region(start, start + size);
    }

    pub fn add_region(&mut self, start: VirtualAddress, end: VirtualAddress) {
        assert!(start <= end);

        let mut current_start = start.as_u64();
        let end = end.as_u64();

        while current_start < end {
            // align blocks based on their start address
            let lowbit = if current_start > 0 {
                current_start & (!current_start + 1)
            // handle case where current_start = 0 so !current_start +1 would overflow
            } else {
                64
            };
            let size = min(
                min(lowbit, previous_power_of_two(end - current_start)),
                1 << (MAX_ORDER - 1),
            );

            let chunk = unsafe { Chunk::new_at_address(VirtualAddress::new(current_start), size) };

            // 0b100 => 2 trailing zeros
            self.buddies[size.trailing_zeros() as usize].push_front(chunk);
            current_start += size;
        }
    }

    const fn min_size() -> usize {
        size_of::<Chunk>()
    }

    // Make sure region is big enough to hold at least the chunk metadata,
    // and big enough to have the alignment required by the layout
    // Due to the way the buddy allocator works, chunk >= layout.align() will
    // be properly aligned
    fn align_layout_size(layout: Layout) -> usize {
        max(
            layout.size().next_power_of_two(),
            max(layout.align(), Self::min_size()),
        )
    }

    /// Alloc a power of two sized range of memory satisfying the layout requirement
    pub unsafe fn alloc(&mut self, layout: Layout) -> Option<NonNull<Chunk>> {
        let size = Self::align_layout_size(layout);

        let class = size.trailing_zeros() as usize;
        // Find first non-empty size class
        for i in class..self.buddies.len() {
            if self.buddies[i].is_empty() {
                continue;
            }

            // split buddies to obtain a chunk closest to the size we want to allocate
            // traverse through multiple size layers if needed
            // Only needed when i > class
            for j in (class + 1..i + 1).rev() {
                if let Some(mut chunk_ptr) = self.buddies[j].pop_front() {
                    // create two buddies of size class n -1 from 1 chunk of size
                    // class n
                    let chunk = chunk_ptr.as_mut();

                    let sz = 1 << (j - 1);
                    let addr = chunk.address();
                    let split_node1 = unsafe { Chunk::new_at_address(addr, sz) };

                    self.buddies[j - 1].push_front(split_node1);

                    let sz = 1 << (j - 1);
                    let addr = chunk.address() + sz;

                    let split_node2 = unsafe { Chunk::new_at_address(addr, sz) };

                    /*
                    println!(
                        "Buddy allocator Split buddy: b1.start: {:#x}, b2.start: {:#x}",
                        region.start(),
                        region.start() + (1 << (j - 1))
                    );
                    */

                    self.buddies[j - 1].push_front(split_node2);
                } else {
                    return None;
                }
            }
            break;
        }

        self.buddies[class].pop_front()
    }

    pub fn dealloc(&mut self, chunk: NonNull<Chunk>) {
        let chunk = unsafe { chunk.as_ref() };
        let mut current_class = chunk.size().trailing_zeros() as usize;
        let mut region = Region::new(chunk.start(), chunk.size());

        // keep merging buddies and moving 1 size class up until not possible anymore
        while current_class < self.buddies.len() {
            let mut buddy = region.clone();
            // buddy addresses differ by exactly 1 bit (the bit corresponding to the bit size)
            // therefore we can get buddy address by simply toggling the size bit
            buddy.set_start(region.start() ^ (1 << current_class));
            // TODO: removing a buddy is O(N). Could be sped up by using e.g. a B-Tree
            match self.buddies[current_class].remove(buddy.start()) {
                Some(_) => {
                    // adjust region for higher size class
                    region.set_start(min(region.start(), buddy.start()));
                    region.set_size(region.size() * 2);

                    current_class += 1;
                }
                None => {
                    let addr = VirtualAddress::new(region.start());
                    let sz = region.size();

                    let chunk = unsafe { Chunk::new_at_address(addr, sz) };

                    self.buddies[current_class].push_front(chunk);
                    break;
                }
            }
        }
    }
}

unsafe impl GlobalAlloc for Locked<BuddyAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match allocator.alloc(layout) {
            Some(chunk) => chunk.as_ptr() as *mut u8,
            None => panic!("Allocator ran out of memory"),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        let size = BuddyAllocator::align_layout_size(layout);
        let chunk = Chunk::new_at_address(VirtualAddress::from_raw_ptr(ptr), size as u64);
        allocator.dealloc(NonNull::new(chunk as *mut Chunk).unwrap())
    }
}
