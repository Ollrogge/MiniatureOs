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
    virtual_base: u64,
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
            virtual_base: vbase,
            info,
            page_table,
            frame_allocator,
            _marker: PhantomData,
        }
    }

    pub fn load_kernel(&mut self, info: &BiosInfo) -> VirtualAddress {
        let kernel = unsafe {
            slice::from_raw_parts(info.kernel.start as *const u8, info.kernel.size as usize)
        };

        let kernel_elf = ElfBinary::new(kernel).expect("Unable to parse kernel elf");

        kernel_elf.load(self).expect("Can't load the binary?");

        VirtualAddress::new(self.virtual_base + kernel_elf.entry_point())
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

            let end_frame: PhysicalFrame<S> = PhysicalFrame::containing_address(
                physical_start_address + header.file_size() - 1u64,
            );

            let start_page: Page<S> = Page::containing_address(VirtualAddress::new(
                self.virtual_base + header.virtual_addr(),
            ));

            let end_page = Page::containing_address(start_page.address + header.mem_size() - 1u64);

            let mut flags = PageTableEntryFlags::PRESENT;
            if !header.flags().is_execute() {
                flags |= PageTableEntryFlags::NO_EXECUTE;
            }
            if header.flags().is_write() {
                flags |= PageTableEntryFlags::WRITABLE;
            }

            if header.file_size() > 0 {
                for frame in PhysicalFrame::range_inclusive(start_frame, end_frame) {
                    let offset = frame - start_frame;
                    let page = start_page + offset / S::SIZE;
                    self.page_table
                        .map_to(frame, page, flags, self.frame_allocator)
                        .expect("Failed to map section");
                }
            } else if header.file_size() == 0 && header.mem_size() > 0 {
                // .bss section handling
                let frame_cnt = header.mem_size().div_ceil(S::SIZE);
                let virtual_start_address =
                    VirtualAddress::new(self.virtual_base + header.offset());
                for page in Page::range_inclusive(start_page, end_page) {
                    let frame = self
                        .frame_allocator
                        .allocate_frame()
                        .expect("Failed to allocate frame for .bss");

                    // zero the frame, (1:1 mapping)
                    let virtual_address = VirtualAddress::new(frame.address.as_u64());
                    let slice = unsafe {
                        slice::from_raw_parts_mut(virtual_address.as_u64() as *mut u8, frame.size())
                    };
                    for e in slice.iter_mut() {
                        *e = 0;
                    }

                    self.page_table
                        .map_to(frame, page, flags, self.frame_allocator)
                        .expect("Failed to map .bss section");
                }
            } else if header.mem_size() > 0 && header.file_size() > 0 {
                // .bss that is partially included in ELF ? never seen it
                unimplemented!(
                    "Load kernel elf: Section with both mem_size and file size bigger 0"
                );
            }
        }

        Ok(())
    }

    fn relocate(&mut self, entry: RelocationEntry) -> Result<(), ElfLoaderErr> {
        unimplemented!("No support for relocations right now");
        Ok(())
    }

    fn load(&mut self, flags: Flags, base: VAddr, region: &[u8]) -> Result<(), ElfLoaderErr> {
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
