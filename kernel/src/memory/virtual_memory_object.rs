use super::{address_space, manager::AllocationStrategy};
use crate::{
    memory::{manager::MemoryManager, MemoryError},
    multitasking::process::Process,
    serial_println,
};
use alloc::vec::Vec;
use core::ops::Drop;
use x86_64::memory::{PageAlignedSize, PageRangeInclusive, PageSize, PhysicalFrame, Size4KiB};
pub trait VirtualMemoryObject {
    fn size(&self) -> PageAlignedSize;
}

// commited page = OS has allocated physical memory for it

#[derive(Default)]
pub struct MemoryBackedVirtualMemoryObject {
    frames: Vec<PhysicalFrame>,
}

impl MemoryBackedVirtualMemoryObject {
    pub fn new(frames: Vec<PhysicalFrame>) -> Self {
        Self { frames }
    }
    // ignore strategy for now. we always allocate frame immediately
    pub fn create_with_frames(
        frames: Vec<PhysicalFrame>,
        _: AllocationStrategy,
    ) -> Result<MemoryBackedVirtualMemoryObject, MemoryError> {
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

impl Drop for MemoryBackedVirtualMemoryObject {
    fn drop(&mut self) {
        serial_println!("Drop MemoryBackedVirtualMemoryObject");
        MemoryManager::the().lock().deallocate_frames(&self.frames);
    }
}
