use crate::memory::{Address, MemoryRegion, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB};
use core::{clone::Clone, iter::Iterator, panic};
/// A trait for types that can allocate a frame of memory.
///
/// # Safety
///
/// The implementer of this trait must guarantee that the `allocate_frame`
/// method returns only unique unused frames. Otherwise, undefined behavior
/// may result from two callers modifying or deallocating the same frame.
pub unsafe trait FrameAllocator<S: PageSize> {
    /// Allocate a frame of the appropriate size and return it if possible.
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<S>>;
}

pub struct BumpFrameAllocator<I: Iterator, D: MemoryRegion> {
    memory_map: I,
    current_region: Option<D>,
    current_frame: PhysicalFrame,
}

impl<I, D> BumpFrameAllocator<I, D>
where
    I: Iterator<Item = D>,
    D: MemoryRegion,
{
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
            current_region,
            current_frame: frame,
        }
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for BumpFrameAllocator<I, D>
where
    I: Iterator<Item = D> + Clone,
    D: MemoryRegion,
{
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        let current_frame = self.current_frame;
        // Only time we cant find any more frames is when we are out of regions
        // in this case this will return None
        let current_region = self.current_region?;

        let mut next_frame = current_frame + 1;

        if !current_region.contains(next_frame.address.as_u64()) {
            while let Some(region) = self.memory_map.next() {
                if !region.is_usable() {
                    continue;
                }

                next_frame =
                    PhysicalFrame::containing_address(PhysicalAddress::new(region.start()));
                self.current_region = Some(region);
                break;
            }
        }

        self.current_frame = next_frame;

        Some(current_frame)
    }
}
