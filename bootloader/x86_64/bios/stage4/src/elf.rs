use crate::println;
use crate::start;
use crate::BumpFrameAllocator;
use crate::PageTable;
use common::BiosInfo;
use core::marker::PhantomData;
use core::slice;
use elfloader::*;
use x86_64::frame_allocator;
use x86_64::memory::Address;
use x86_64::memory::PhysicalAddress;
use x86_64::memory::PhysicalFrame;
use x86_64::memory::VirtualAddress;
use x86_64::memory::{Page, PageSize};
use x86_64::paging::PageTableEntryFlags;
use x86_64::paging::{FourLevelPageTable, Mapper};
use x86_64::{frame_allocator::FrameAllocator, memory::Size4KiB};

pub struct KernelLoader<'a, M, A, S> {
    vbase: u64,
    info: &'a BiosInfo<'a>,
    page_table: &'a mut M,
    frame_allocator: &'a mut A,
    _marker: PhantomData<S>,
}

impl<'a, M, A, S> KernelLoader<'a, M, A, S>
where
    M: Mapper<S>,
    A: FrameAllocator<S>,
    S: PageSize,
{
    pub fn new(
        vbase: u64,
        info: &'a BiosInfo,
        page_table: &'a mut M,
        frame_allocator: &'a mut A,
    ) -> Self {
        Self {
            vbase,
            info,
            page_table,
            frame_allocator,
            _marker: PhantomData,
        }
    }

    pub fn load_kernel(&mut self, info: &BiosInfo) {
        let kernel = unsafe {
            slice::from_raw_parts(info.kernel.start as *const u8, info.kernel.size as usize)
        };

        let kernel_elf = ElfBinary::new(kernel).expect("Unable to parse kernel elf");

        kernel_elf.load(self).expect("Can't load the binary?");
    }
}

impl<'a, M, A, S> ElfLoader for KernelLoader<'a, M, A, S>
where
    M: Mapper<S>,
    A: FrameAllocator<S>,
    S: PageSize,
{
    fn allocate(&mut self, load_headers: LoadableHeaders) -> Result<(), ElfLoaderErr> {
        for header in load_headers {
            println!(
                "allocate base = {:#x} size = {:#x} flags = {}",
                header.virtual_addr(),
                header.mem_size(),
                header.flags()
            );

            let physical_start_address =
                PhysicalAddress::new(self.info.kernel.start + header.offset());
            let start_frame: PhysicalFrame<S> =
                PhysicalFrame::containing_address(physical_start_address);

            let end_frame: PhysicalFrame<S> =
                PhysicalFrame::containing_address(physical_start_address + header.file_size());

            let start_page: Page<S> =
                Page::containing_address(VirtualAddress::new(self.vbase + header.virtual_addr()));

            let mut flags = PageTableEntryFlags::WRITABLE;
            if !header.flags().is_execute() {
                flags |= PageTableEntryFlags::NO_EXECUTE;
            }
            if header.flags().is_write() {
                flags |= PageTableEntryFlags::WRITABLE;
            }

            /* TODO: why do I have to map both the file_size and the mem_size for a .bss section ?  */
            println!(
                "Header: mem_sz:{} file_sz:{}, Frame: start:{} end:{}",
                header.mem_size(),
                header.file_size(),
                start_frame,
                end_frame
            );

            for frame in PhysicalFrame::range_inclusive(start_frame, end_frame) {
                let offset = frame - start_frame;
                let page = start_page + offset;
                println!("bla: {:#x}", page.address.as_u64());
                self.page_table
                    .map_to(frame, page, flags, self.frame_allocator)
                    .expect("Failed to map section");
            }

            if header.mem_size() > header.file_size() {
                println!("Test: {}, {}", start_frame, end_frame);
            }
        }

        Ok(())
    }

    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), ElfLoaderErr> {
        println!("Relocate called");
        unimplemented!("No support for relocations right now");
        Ok(())
    }

    fn load(&mut self, flags: Flags, base: VAddr, region: &[u8]) -> Result<(), ElfLoaderErr> {
        let start = self.vbase + base;
        let end = self.vbase + base + region.len() as u64;
        println!(
            "load region into = {:#x} -- {:#x} -- {:#x}",
            start, end, base
        );
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
