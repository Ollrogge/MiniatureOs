use alloc::collections::BTreeSet;
use core::array;
use x86_64::memory::{PhysicalMemoryRegion, PhysicalMemoryRegionType};
// todo: make a frame_allocators directory
//  - lib (or mod idk) file contains the trait def
//  - then have 1 file for buddy 1 for Bump
pub struct BuddyFrameAllocator<const ORDER: usize = 32> {
    free_list: [BTreeSet<usize>; ORDER],
}

impl<const ORDER: usize> BuddyFrameAllocator<ORDER> {
    pub fn new() -> Self {
        Self {
            free_list: array::from_fn(|_| BTreeSet::default()),
        }
    }

    pub fn from_memory_map<I>(memory_map: I) -> Self
    where
        I: Iterator<Item = PhysicalMemoryRegion>,
    {
        //for region in memory_map.iter() {}
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BuddyFrameAllocator;
    use x86_64::memory::{PhysicalMemoryRegion, PhysicalMemoryRegionType};
    #[test]
    fn test_from_memory_map() {
        let memory_map = [
            PhysicalMemoryRegion::new(0, 0x1000, PhysicalMemoryRegionType::Free),
            PhysicalMemoryRegion::new(0x1000, 0x1000, PhysicalMemoryRegionType::Reserved),
            PhysicalMemoryRegion::new(0x4000, 0x8000, PhysicalMemoryRegionType::Free),
        ];

        let allocator = BuddyFrameAllocator::<32>::from_memory_map(memory_map.iter());
    }
}
