use crate::{
    memory::{
        Address, FrameAllocator, MemoryRegion, PageSize, PhysicalAddress, PhysicalFrame,
        PhysicalMemoryRegion, Size4KiB,
    },
    serial_println,
};
use core::{
    clone::Clone,
    iter::{Iterator, Peekable},
    panic,
};

/// Very simple bump allocator. Allocates memory linearly and only keeps track
/// of the number of allocated bytes and the number of allocations.
///     - linearity is good for the bootloader since this won't fragment the
///     the physical address space. With this we can build and pass a simple
///     memory map to the kernel
/// Can only free all memory at once.
// https://os.phil-opp.com/allocator-designs/#bump-allocator
pub struct BumpFrameAllocator<I: Iterator<Item = D>, D: MemoryRegion> {
    memory_map: Peekable<I>,
    next: usize,
}

impl<I, D> BumpFrameAllocator<I, D>
where
    I: Iterator<Item = D> + Clone,
    D: MemoryRegion,
{
    pub fn new(memory_map: Peekable<I>) -> Self {
        Self {
            memory_map,
            next: 0,
        }
    }
    // The frame passed to this function MUST be valid
    pub fn new_starting_at(frame: PhysicalFrame, mut memory_map: Peekable<I>) -> Self {
        // todo: this assmumes memory map is sorted
        // advance iterator until reaching the frame to start at
        while let Some(region) = memory_map.peek_mut() {
            if region.contains(frame.address.as_u64()) {
                if !region.is_usable() {
                    panic!("Tried to initialize allocator at unusable address");
                }

                // adjust region start to begin at `frame`
                region.set_start(frame.start());
                break;
            }
            memory_map.next();
        }

        Self {
            memory_map,
            next: 0,
        }
    }

    pub fn max_physical_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.memory_map.clone().map(|r| r.end()).max().unwrap())
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame> {
        let usable_regions = self.memory_map.clone().filter(|r| r.is_usable());
        let addr_ranges = usable_regions.map(|r| r.start()..r.end());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(Size4KiB::SIZE));
        frame_addresses.map(|addr| PhysicalFrame::containing_address(PhysicalAddress::new(addr)))
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

    // not implemented by bump allocator
    fn deallocate_frame(&mut self, _: PhysicalFrame<Size4KiB>) {}
}
