use crate::println;
use core::slice;
use x86_64::{frame_allocator::FrameAllocator, memory::Size4KiB};
use xmas_elf::ElfFile;

use crate::BumpFrameAllocator;
use common::BiosInfo;

pub fn map_kernel<S>(pml4t: &mut PageTable, info: &BiosInfo, allocator: &mut S)
where
    S: FrameAllocator<Size4KiB>,
{
    let kernel =
        unsafe { slice::from_raw_parts(info.kernel.start as *const u8, info.kernel.size as usize) };

    let kernel_elf = ElfFile::new(kernel).expect("Unable to parse kernel elf");
}
