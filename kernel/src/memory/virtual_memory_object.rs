use super::manager::AllocationStrategy;
use crate::memory::{manager::MemoryManager, MemoryError};
use alloc::vec::Vec;
use x86_64::memory::{PageAlignedSize, PageSize, PhysicalFrame, Size4KiB};
pub trait VirtualMemoryObject {
    fn size(&self) -> PageAlignedSize;
}

// commited page = OS has allocated physical memory for it

pub struct MemoryBackedVirtualMemoryObject {
    frames: Vec<PhysicalFrame>,
}

impl MemoryBackedVirtualMemoryObject {
    pub fn new(frames: Vec<PhysicalFrame>) -> Self {
        Self { frames }
    }
    // ignore strategy for now. we always allocate frame immediately
    pub fn create(
        memory_manager: &mut MemoryManager,
        size: PageAlignedSize,
        _: AllocationStrategy,
    ) -> Result<MemoryBackedVirtualMemoryObject, MemoryError> {
        let frames = memory_manager.try_allocate_frames(size.in_pages())?;

        Ok(Self { frames })
    }

    pub fn frames(&self) -> &Vec<PhysicalFrame> {
        &self.frames
    }
}

impl VirtualMemoryObject for MemoryBackedVirtualMemoryObject {
    fn size(&self) -> PageAlignedSize {
        PageAlignedSize::new(self.frames.len() * Size4KiB::SIZE)
    }
}
