use core::{clone::Clone, iter::Iterator, panic};
use x86_64::{
    memory::{
        Address, FrameAllocator, MemoryRegion, PageSize, PhysicalAddress, PhysicalFrame,
        PhysicalMemoryRegion, Size4KiB,
    },
    println,
};

/// Very simple bump allocator. Allocates memory linearly and only keeps track
/// of the number of allocated bytes and the number of allocations.
///     - linearity is good for the bootloader since this won't fragment the
///     the physical address space. With this we can build and pass a simple
///     memory map to the kernel
/// Can only free all memory at once.
// https://os.phil-opp.com/allocator-designs/#bump-allocator
pub struct BumpFrameAllocator<I: Iterator, D: MemoryRegion> {
    memory_map: I,
    next: usize,
}

impl<I, D> BumpFrameAllocator<I, D>
where
    I: Iterator<Item = D> + Clone,
    D: MemoryRegion,
{
    // The frame passed to this function MUST be valid
    pub fn new_starting_at(frame: PhysicalFrame, mut memory_map: I) -> Self {
        // todo: this assmumes memory map is sorted
        let mut current_region = None;
        while let Some(region) = memory_map.next() {
            if region.contains(frame.address.as_u64()) {
                if !region.is_usable() {
                    panic!("Tried to initialize allocator at unusable address");
                }
                current_region = Some(region);
                break;
            }
        }
        Self {
            memory_map: memory_map,
            next: 0,
        }
    }

    pub fn max_physical_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.memory_map.clone().map(|r| r.end()).max().unwrap())
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame> {
        let usable_regions = self.memory_map.filter(|r| r.is_usable());
        let addr_ranges = usable_regions.map(|r| r.start()..r.end());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(PageSize::SIZE));
        frame_addresses.map(|addr| PhysicalFrame::containing_address(addr))
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for BumpFrameAllocator<I, D>
where
    I: Iterator<Item = D> + Clone,
    D: MemoryRegion,
{
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
