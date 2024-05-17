extern crate alloc;
use alloc::collections::BTreeSet;
use core::{array, cmp::min, mem::MaybeUninit, ops::DerefMut, ptr::NonNull};
use x86_64::{
    memory::{MemoryRegion, PhysicalMemoryRegion, PhysicalMemoryRegionType},
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

// max order is 1 MiB => max buddy size is 512kib
const MAX_ORDER: usize = 20;

const LIST_SIZE: usize = 512;

fn previous_power_of_two(num: u64) -> u64 {
    1 << (u64::BITS - num.leading_zeros() - 1)
}

trait LinkedListTrait {
    fn pop_front(&mut self) -> Option<&mut Node>;
    fn remove(&mut self, region_start: PhysicalMemoryRegion) -> Option<&mut Node>;
    fn front(&self) -> Option<&Node>;
    fn is_empty(&self) -> bool;
    fn front_mut(&mut self) -> Option<&mut Node>;
}

#[derive(Clone, Copy)]
struct Node {
    next: Option<NonNull<Node>>,
    region: PhysicalMemoryRegion,
}

impl Node {
    pub fn reset(&mut self) {
        self.next = None;
        self.region = PhysicalMemoryRegion::default();
    }
}

// Linked list providing its own storage for the nodes
struct LinkedListWithStorage {
    // You can think of MaybeUninit<T> as being a bit like Option<T>
    // but without any of the run-time tracking and without any of the safety checks.
    pub nodes: [MaybeUninit<Node>; LIST_SIZE],
    pub head: Option<NonNull<Node>>,
}

impl LinkedListWithStorage {
    pub fn new() -> Self {
        const UNINIT: MaybeUninit<Node> = MaybeUninit::uninit();

        let mut list = Self {
            nodes: [UNINIT; LIST_SIZE],
            head: None,
        };

        for node in &mut list.nodes {
            let node_ptr: *mut Node = node.as_mut_ptr();
            let node_ref = unsafe { &mut *node_ptr };

            node_ref.next = list.head;
            list.head = Some(NonNull::new(node_ptr).unwrap());
        }

        list
    }

    fn push_front(&mut self, block: *mut Node) {
        unsafe {
            (*block).next = self.head;
        }
        self.head = Some(NonNull::new(block).unwrap());
    }
}

impl LinkedListTrait for LinkedListWithStorage {
    fn pop_front(&mut self) -> Option<&mut Node> {
        if let Some(mut block) = self.head.take() {
            self.head = unsafe { block.as_mut().next.take() };
            Some(unsafe { block.as_mut() })
        } else {
            None
        }
    }

    fn remove(&mut self, _: PhysicalMemoryRegion) -> Option<&mut Node> {
        None
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn front(&self) -> Option<&Node> {
        self.head
            .as_ref()
            .map(|non_null| unsafe { non_null.as_ref() })
    }

    fn front_mut(&mut self) -> Option<&mut Node> {
        self.head
            .as_mut()
            .map(|non_null| unsafe { non_null.as_mut() })
    }
}

#[derive(Clone, Copy)]
struct LinkedList {
    // You can think of MaybeUninit<T> as being a bit like Option<T>
    // but without any of the run-time tracking and without any of the safety checks.
    pub head: Option<NonNull<Node>>,
}

impl LinkedList {
    pub fn new() -> Self {
        Self { head: None }
    }

    fn push_front(&mut self, block: &mut Node) {
        block.next = self.head;
        self.head = Some(NonNull::new(block).unwrap());
    }
}

impl LinkedListTrait for LinkedList {
    fn pop_front(&mut self) -> Option<&mut Node> {
        if let Some(mut block) = self.head.take() {
            self.head = unsafe { block.as_mut().next.take() };
            Some(unsafe { block.as_mut() })
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn front(&self) -> Option<&Node> {
        self.head
            .as_ref()
            .map(|non_null| unsafe { non_null.as_ref() })
    }

    fn front_mut(&mut self) -> Option<&mut Node> {
        self.head
            .as_mut()
            .map(|non_null| unsafe { non_null.as_mut() })
    }

    fn remove(&mut self, region: PhysicalMemoryRegion) -> Option<&mut Node> {
        let mut last_node: Option<*mut Node> = None;
        let mut cur_node = self.front_mut().map(|node| node as *mut Node);

        while let Some(node_ptr) = cur_node {
            let node = unsafe { &mut *node_ptr };
            if node.region.start() == region.start() {
                // If the node to remove is found, update the links
                match last_node {
                    Some(last_node_ptr) => unsafe { (*last_node_ptr).next = node.next },
                    None => self.head = node.next,
                }

                // Return the mutable reference to the removed node
                return Some(node);
            }

            // Move to the next node
            last_node = cur_node;
            cur_node = node.next.as_mut().map(|non_null| non_null.as_ptr());
        }

        None
    }
}

pub struct BuddyFrameAllocator {
    buddies: [LinkedList; MAX_ORDER],
    node_pool: LinkedListWithStorage,
}

impl<'a> BuddyFrameAllocator {
    pub fn new() -> Self {
        Self {
            buddies: [LinkedList::new(); MAX_ORDER],
            node_pool: LinkedListWithStorage::new(),
        }
    }

    pub fn init<I>(&mut self, memory_map: I)
    where
        I: Iterator<Item = PhysicalMemoryRegion>,
    {
        for region in memory_map {
            if !region.is_usable() {
                continue;
            }

            let start = region.start();
            let end = region.end() - 1;

            self.add_frame(start, end);
        }
    }

    pub fn add_frame(&mut self, start: u64, end: u64) {
        assert!(start <= end);

        let mut current_start = start;

        while current_start < end {
            let lowbit = if current_start > 0 {
                current_start & (!current_start + 1)
            } else {
                64
            };
            let size = min(
                min(lowbit, previous_power_of_two(end - current_start)),
                1 << (MAX_ORDER - 1),
            );

            let node = self
                .node_pool
                .pop_front()
                .expect("Buddy allocator memory pool exhausted");

            // size not needed due to buddy array but lets make it clear
            node.region.set_size(size);
            node.region.set_start(current_start);

            // 200 => 2 trailing zeros
            self.buddies[size.trailing_zeros() as usize].push_front(node);
            current_start += size;
        }
    }

    pub fn alloc(&mut self, size: u64) -> Option<PhysicalMemoryRegion> {
        let size = size.next_power_of_two();
        match self.alloc_power_of_two(size) {
            // give back node to pool
            Some(node) => {
                let region = node.region;

                node.reset();

                let node_ptr = node as *mut Node;
                self.node_pool.push_front(node_ptr);

                Some(region)
            }
            None => None,
        }
    }

    /// Alloc of power of two sized frame. The frame will have alignment equal to the size
    fn alloc_power_of_two(&mut self, size: u64) -> Option<&mut Node> {
        let class = size.trailing_zeros() as usize;
        // Find first non-empty size class
        for i in class..self.buddies.len() {
            if self.buddies[i].is_empty() {
                continue;
            }

            // split buddies. Only when i > class
            for j in (class + 1..i + 1).rev() {
                if let Some(node) = self.buddies[j].pop_front() {
                    let node = node.clone();
                    let region = &node.region;

                    let split_node1 = self.node_pool.pop_front().expect("node pool exhausted");

                    split_node1.region.set_start(region.start());
                    split_node1.region.set_size(1 << (j - 1));

                    self.buddies[j - 1].push_front(split_node1);

                    let split_node2 = self.node_pool.pop_front().expect("node pool exhausted");
                    split_node2
                        .region
                        .set_start(region.start() + (1 << (j - 1)));
                    split_node2.region.set_size(1 << (j - 1));

                    println!("Buddy allocator Split buddy: {:#x}", region.start(),);

                    self.buddies[j - 1].push_front(split_node2);
                } else {
                    return None;
                }
            }
            break;
        }

        self.buddies[class].pop_front()
    }

    pub fn dealloc(&mut self, region: PhysicalMemoryRegion) {
        assert!(region.size() % 2 == 0);
        self.dealloc_power_of_two(region);
    }

    fn dealloc_power_of_two(&mut self, region: PhysicalMemoryRegion) {
        let mut current_class = region.size().trailing_zeros() as usize;

        let mut region = region;
        while current_class < self.buddies.len() {
            let mut buddy = region.clone();
            // buddy addresses differ by exactly 1 bit (the bit corresponding to the bit size)
            // therefore we can get buddy address by simply toggling the size bit
            buddy.set_start(region.start() ^ (1 << current_class));
            // Only have to remove the buddy since dealloc => insert into buddy list
            match self.buddies[current_class].remove(buddy) {
                Some(buddy_node) => {
                    region.set_start(min(region.start(), buddy.start()));
                    region.set_size(region.size() * 2);

                    // give back node to pool
                    let node_ptr = buddy_node as *mut Node;
                    self.node_pool.push_front(node_ptr);

                    current_class += 1;
                }
                None => {
                    let node = self.node_pool.pop_front().expect("Node pool exhausted");
                    node.region = region;

                    self.buddies[current_class].push_front(node);
                    break;
                }
            }
        }
    }
}
