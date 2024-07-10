use super::manager::{AllocationStrategy, MemoryError};
use crate::memory::manager::MemoryManager;
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
    // ignore startegy for now. we always allocate frame immediately
    pub fn create(
        size: usize,
        strategy: AllocationStrategy,
    ) -> Result<MemoryBackedVirtualMemoryObject, MemoryError> {
        assert!(
            size % Size4KiB::SIZE == 0,
            "MemoryBackedVirtualMemoryObject needs to be multiple of page size"
        );

        let frames = MemoryManager::the()
            .lock()
            .try_allocate_frames(size / Size4KiB::SIZE)?;

        Ok(Self { frames })
    }
}

impl VirtualMemoryObject for MemoryBackedVirtualMemoryObject {
    fn size(&self) -> PageAlignedSize {
        PageAlignedSize::new(self.frames.len() * Size4KiB::SIZE)
    }
}
