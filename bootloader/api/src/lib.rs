#![no_std]
use x86_64::memory::Region;
pub struct BootInfo {
    pub kernel: Region,
}

impl BootInfo {
    pub fn new(kernel: Region) -> Self {
        Self { kernel }
    }
}
