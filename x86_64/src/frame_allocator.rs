use crate::memory::{Address, MemoryRegion, PhysicalAddress, PhysicalFrame};
use crate::memory::{PageSize, Size4KiB};
use core::clone::Clone;
use core::iter::{Iterator, Peekable};
use core::panic;

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

pub struct BumpFrameAllocator<I: Iterator> {
    memory_map: Peekable<I>,
    current_frame: Option<PhysicalFrame>,
}

impl<I, D> BumpFrameAllocator<I>
where
    I: Iterator<Item = D>,
    D: MemoryRegion,
{
    pub fn new_starting_at(frame: PhysicalFrame, mut memory_map: Peekable<I>) -> Self {
        while let Some(region) = memory_map.peek() {
            if region.contains(frame.address.as_u64()) {
                break;
            }

            memory_map.next();
        }
        Self {
            memory_map: memory_map.into(),
            current_frame: Some(frame),
        }
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for BumpFrameAllocator<I>
where
    I: Iterator<Item = D> + Clone,
    I::Item: MemoryRegion,
{
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<Size4KiB>> {
        let current_frame = self.current_frame?;
        let next_frame = current_frame + 1;

        // Either return the next frame in the current memory region or
        // the first frame of the next memory region
        if let Some(region) = self.memory_map.peek() {
            if region.contains(next_frame.address.as_u64()) {
                self.current_frame = Some(next_frame);
            } else {
                self.memory_map.next()?;
                self.current_frame = self.memory_map.peek().map(|region| {
                    PhysicalFrame::containing_address(PhysicalAddress::new(region.start()))
                });
            }
        }

        Some(current_frame)
    }
}
