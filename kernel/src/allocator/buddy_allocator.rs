//! This module implements a buddy frame allocator
//!
extern crate alloc;
use super::Locked;
use crate::{println, serial_print, serial_println};
use alloc::{
    alloc::{GlobalAlloc, Layout},
    borrow::ToOwned,
};
use core::{
    cmp::{max, min},
    mem,
    mem::MaybeUninit,
    pin::Pin,
    ptr,
    ptr::NonNull,
};
use util::intrusive_linked_list::{BoxAt, IntrusiveLinkedList, Linked, Links};
use x86_64::memory::{
    Address, FrameAllocator, MemoryRegion, PageSize, PhysicalAddress, PhysicalFrame,
    PhysicalMemoryRegion, PhysicalMemoryRegionType, Region, Size2MiB, Size4KiB, VirtualAddress,
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

pub struct Chunk {
    links: Links<Chunk>,
    // Technically not nedded but maybe interesting for statistics
    size: usize,
}

unsafe impl Send for Chunk {}

impl Chunk {
    pub fn new(size: usize) -> Self {
        Self {
            links: Links::new(),
            size,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn start(&self) -> u64 {
        self as *const Self as u64
    }

    pub fn address(&self) -> usize {
        self as *const Self as usize
    }
}

unsafe impl Linked<Links<Chunk>> for Chunk {
    type Handle = BoxAt<Self>;

    fn into_ptr(handle: Self::Handle) -> NonNull<Chunk> {
        NonNull::from(BoxAt::leak(handle))
    }

    unsafe fn from_ptr(ptr: NonNull<Chunk>) -> Self::Handle {
        BoxAt::from_raw(ptr.as_ptr())
    }

    unsafe fn links(target: NonNull<Chunk>) -> NonNull<Links<Chunk>> {
        let links = ptr::addr_of_mut!((*target.as_ptr()).links);
        NonNull::new_unchecked(links)
    }
}

pub struct BuddyAllocator {
    buddies: [Option<IntrusiveLinkedList<Chunk>>; MAX_ORDER],
}

impl<'a> BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            buddies: [const { None }; MAX_ORDER],
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

            let chunk = BoxAt::new(
                usize::try_from(current_start).unwrap(),
                Chunk::new(size as usize),
            );

            // 0b100 => 2 trailing zeros
            self.buddies[size.trailing_zeros() as usize]
                .get_or_insert_with(IntrusiveLinkedList::new)
                .push_front(chunk);
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
        .next_power_of_two()
    }

    /// Alloc a power of two sized range of memory satisfying the layout requirement
    pub unsafe fn alloc(&mut self, layout: Layout) -> Option<BoxAt<Chunk>> {
        let size = Self::align_layout_size(layout);
        assert_eq!(size % 2, 0);
        let class = size.trailing_zeros() as usize;
        // Find first non-empty size class
        for i in class..self.buddies.len() {
            if self.buddies[i]
                .get_or_insert_with(IntrusiveLinkedList::new)
                .is_empty()
            {
                continue;
            }

            // split buddies to obtain a chunk closest to the size we want to allocate
            // traverse through multiple size layers if needed (Only required when i > class)
            for j in (class + 1..i + 1).rev() {
                if let Some(chunk) = self.buddies[j]
                    .get_or_insert_with(IntrusiveLinkedList::new)
                    .pop_front()
                {
                    let list = self.buddies[j - 1].get_or_insert_with(IntrusiveLinkedList::new);

                    // create two buddies of size class n -1 from 1 chunk of size
                    // class n
                    let addr = chunk.address();
                    let sz = 1 << (j - 1);

                    let split_node1 = BoxAt::new(addr, Chunk::new(sz));
                    list.push_front(split_node1);

                    let split_node2 = BoxAt::new(addr + sz, Chunk::new(sz));
                    list.push_front(split_node2);
                } else {
                    return None;
                }
            }
            break;
        }

        self.buddies[class].as_mut().unwrap().pop_front()
    }

    pub fn dealloc(&mut self, chunk: BoxAt<Chunk>) {
        let mut current_class = chunk.size().trailing_zeros() as usize;

        // keep track of the region we are merging
        let mut region = Region::new(chunk.start(), chunk.size());

        // keep merging buddies and moving 1 size class up until not possible anymore
        while current_class < self.buddies.len() {
            // buddy addresses differ by exactly 1 bit (the bit corresponding to the bit size)
            // therefore we can get buddy address by simply toggling the size bit
            let buddy: BoxAt<Chunk> = unsafe {
                BoxAt::from_address(usize::try_from(region.start() ^ (1 << current_class)).unwrap())
            };

            let buddy_start = buddy.start();

            match self.buddies[current_class]
                .get_or_insert_with(IntrusiveLinkedList::new)
                .remove_checked(unsafe { NonNull::new_unchecked(BoxAt::leak(buddy)) })
            {
                // merge two buddies
                Some(_) => {
                    // adjust region for higher size class
                    region.set_start(min(region.start(), buddy_start));
                    region.set_size(region.size() * 2);
                    current_class += 1;
                }
                None => {
                    let chunk = BoxAt::new(
                        usize::try_from(region.start()).unwrap(),
                        Chunk::new(region.size()),
                    );

                    self.buddies[current_class]
                        .get_or_insert_with(IntrusiveLinkedList::new)
                        .push_front(chunk);
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
            Some(chunk) => BoxAt::into_raw(chunk) as *mut u8,
            None => panic!("Allocator ran out of memory"),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        let sz = BuddyAllocator::align_layout_size(layout);
        // initialize chunk at address
        let address = ptr as usize;
        let chunk = BoxAt::new(address, Chunk::new(sz));
        allocator.dealloc(chunk);
    }
}
