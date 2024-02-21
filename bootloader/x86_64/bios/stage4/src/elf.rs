use crate::{start, BumpFrameAllocator, PageTable};
use common::BiosInfo;
use core::{cmp, marker::PhantomData, mem, ops::Add, ptr, slice};
use elfloader::{arch::x86_64::RelocationTypes, *};
use x86_64::{
    frame_allocator,
    frame_allocator::FrameAllocator,
    memory::{Address, Page, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB, VirtualAddress},
    paging::{FourLevelPageTable, Mapper, PageTableEntryFlags, Translator},
    println,
};

pub struct KernelLoader<'a, M, A, S> {
    virtual_base: u64,
    info: &'a BiosInfo,
    page_table: &'a mut M,
    frame_allocator: &'a mut A,
    _marker: PhantomData<S>,
}

impl<'a, M, A, S> KernelLoader<'a, M, A, S>
where
    M: Mapper<S> + Translator<S>,
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

    // https://dram.page/p/relative-relocs-explained/
    // Basically means: Please fill in the value of (virtual_base + addend) at offset from base of executable
    fn handle_relative_relocation(&mut self, entry: RelocationEntry) {
        let value = self.virtual_base
            + entry
                .addend
                .expect("Relative relocation: addend value = None");

        // the relocation may span two pages
        // (e.g. 4 bytes of value on page A and 4 bytes on page B)
        let virtual_address = VirtualAddress::new(self.virtual_base + entry.offset);
        let start_page = Page::containing_address(virtual_address);
        let end_page = Page::containing_address(virtual_address + mem::size_of::<u64>());
        for page in Page::range_inclusive(start_page, end_page) {
            // entry.offset if relative to virtual base, so we need to first map
            // the page corresponding to virtual base to its physical frame and then
            // calculate the correct offset
            let frame = self
                .page_table
                .translate(page)
                .expect("Relative relocation: Failed to map page to frame");
            let end_of_page = page.address + page.size() - 1;
            let offset = (cmp::min(virtual_address, end_of_page) - page.address.as_u64()).as_u64();

            let address = VirtualAddress::new(frame.address.as_u64() + offset);
            let ptr = address.as_mut_ptr();
            unsafe { ptr::write(ptr, value.to_ne_bytes()) };
        }
    }
}

impl<'a, M, A, S> ElfLoader for KernelLoader<'a, M, A, S>
where
    M: Mapper<S> + Translator<S>,
    A: FrameAllocator<S>,
    S: PageSize,
{
    // we just need allocate and not load since we align down the addresses such
    // that accesses will still work.
    fn allocate(&mut self, load_headers: LoadableHeaders) -> Result<(), ElfLoaderErr> {
        for header in load_headers {
            println!(
                "Kernel elf: allocate segment at {:#x}, size = {:#x}, flags = {}",
                header.virtual_addr() + self.virtual_base,
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

            // TODO: the approach of loading the kernel elf and binary blob into
            // memory has a potential security problem: This way different segments
            // can share a physical frame. When mapped to a page with permissions,
            // the page will then contain data of another segment with different
            // permissions
            //
            // To fix this, one would need to allocate explicit frames in this
            // section for each segment.
            //
            // Cant get around the loading binary blob into memory thing for now ig,
            // since we only have access to BIOS disk firmware in lower stages

            if header.file_size() > 0 {
                for frame in PhysicalFrame::range_inclusive(start_frame, end_frame) {
                    let offset = frame - start_frame;
                    let page: Page<S> = start_page + offset;
                    self.page_table
                        .map_to(frame, page, flags, self.frame_allocator)
                        .expect("Failed to map section");
                }
            } else if header.file_size() == 0 && header.mem_size() > 0 {
                // .bss section handling
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
        match entry.rtype {
            RelocationType::x86_64(typ) => match typ {
                RelocationTypes::R_AMD64_RELATIVE => {
                    self.handle_relative_relocation(entry);
                }
                _ => panic!("Unhandled relocation type: {:?}", typ),
            },
            _ => panic!("Expected x86_64 relocation type but got x86 relocation type"),
        }
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
