use crate::println;
use crate::BumpFrameAllocator;
use crate::PageTable;
use common::BiosInfo;
use core::slice;
use elfloader::*;
use x86_64::{frame_allocator::FrameAllocator, memory::Size4KiB};

pub const MiB: usize = 0x100000;
pub const KiB: usize = 0x400;
pub const GiB: usize = 0x40000000;

// hardcoded for now;
const KERNEL_VIRTUAL_BASE: u64 = 0xffffffff80000000;
const KERNEL_STACK_TOP: u64 = 0xffffffff00000000;
const KERNEL_STACK_SIZE: usize = 2 * KiB;

struct KernelLoader {
    vbase: u64,
}

impl KernelLoader {
    pub fn new(vbase: u64) -> Self {
        Self { vbase }
    }
}

impl ElfLoader for KernelLoader {
    fn allocate(&mut self, load_headers: LoadableHeaders) -> Result<(), ElfLoaderErr> {
        for header in load_headers {
            println!(
                "allocate base = {:#x} size = {:#x} flags = {}",
                header.virtual_addr(),
                header.mem_size(),
                header.flags()
            );
        }
        Ok(())
    }

    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), ElfLoaderErr> {
        println!("Relocate called");
        Ok(())
    }

    fn load(&mut self, flags: Flags, base: VAddr, region: &[u8]) -> Result<(), ElfLoaderErr> {
        let start = self.vbase + base;
        let end = self.vbase + base + region.len() as u64;
        println!("load region into = {:#x} -- {:#x}", start, end);
        Ok(())
    }

    fn tls(
        &mut self,
        tdata_start: VAddr,
        _tdata_length: u64,
        total_size: u64,
        _align: u64,
    ) -> Result<(), ElfLoaderErr> {
        let tls_end = tdata_start + total_size;
        println!(
            "Initial TLS region is at = {:#x} -- {:#x}",
            tdata_start, tls_end
        );
        Ok(())
    }
}

pub fn map_kernel<S>(pml4t: &mut PageTable, info: &BiosInfo, allocator: &mut S)
where
    S: FrameAllocator<Size4KiB>,
{
    let kernel =
        unsafe { slice::from_raw_parts(info.kernel.start as *const u8, info.kernel.size as usize) };

    let kernel_elf = ElfBinary::new(kernel).expect("Unable to parse kernel elf");
    let mut loader = KernelLoader::new(KERNEL_VIRTUAL_BASE);
    kernel_elf
        .load(&mut loader)
        .expect("Can't load the binary?");
}
