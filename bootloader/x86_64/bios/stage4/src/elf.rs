use common::BiosInfo;
use core::{cmp, marker::PhantomData, mem, ops::Add, ptr, slice};
use elfloader::{arch::x86_64::RelocationTypes, *};
use x86_64::{
    memory::{Address, Page, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB, VirtualAddress},
    paging::{
        mapped_page_table::MappedPageTable, FrameAllocator, Mapper, MapperAllSizes, PageTable,
        PageTableEntryFlags, Translator, TranslatorAllSizes,
    },
    println,
};

// TODO: move this functionality to a more general util crate since I will
// need it to load elfs for the kernel as well
// also TODO: remove dependency to elfloader

pub struct KernelLoader<'a, M, A> {
    virtual_base: u64,
    info: &'a BiosInfo,
    page_table: &'a mut M,
    frame_allocator: &'a mut A,
}

impl<'a, M, A> KernelLoader<'a, M, A>
where
    M: MapperAllSizes + TranslatorAllSizes,
    A: FrameAllocator<Size4KiB>,
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
    // Basically means: Please fill in the value of (virtual_base + addend)
    // at offset from base of executable
    fn handle_relative_relocation(&mut self, entry: RelocationEntry) {
        let value_bytes = (self.virtual_base
            + entry
                .addend
                .expect("Relative relocation: addend value = None"))
        .to_ne_bytes();

        // the relocation may span two pages
        // (e.g. 4 bytes of value on page A and 4 bytes on page B)
        let virtual_address = VirtualAddress::new(self.virtual_base + entry.offset);
        let start_page = Page::containing_address(virtual_address);
        let end_page = Page::containing_address(virtual_address + value_bytes.len());
        let mut bytes_written = 0;
        for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
            // entry.offset if relative to virtual base, so we need to first map
            // the page corresponding to virtual base to its physical frame and then
            // calculate the correct offset
            let frame = self
                .page_table
                .translate(page)
                .expect("Relative relocation: Failed to translate to frame");

            // we are on the first page
            let offset = if virtual_address > page.address {
                (virtual_address - page.address) as usize
            // we are on the second page
            } else {
                0
            };

            // Write spans two pages, so calculate amount of bytes for first page
            let bytes_to_write = if virtual_address + value_bytes.len() > page.end() {
                (page.end() - virtual_address) as usize
            // Either we write the full 8 bytes, or the rest of the value on the second page
            } else {
                value_bytes.len() - bytes_written
            };

            let ptr =
                VirtualAddress::new(frame.address.as_u64() + offset as u64).as_mut_ptr::<u8>();

            // Calculate the slice of value_bytes to write
            let write_slice = &value_bytes[bytes_written..(bytes_written + bytes_to_write)];

            let address =
                VirtualAddress::new(frame.address.as_u64() + offset as u64).as_mut_ptr::<u8>();

            unsafe { ptr::copy_nonoverlapping(write_slice.as_ptr(), address, bytes_to_write) };

            bytes_written += bytes_to_write;
        }
    }
}

impl<'a, M, A> ElfLoader for KernelLoader<'a, M, A>
where
    M: MapperAllSizes + TranslatorAllSizes,
    A: FrameAllocator<Size4KiB>,
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
            let start_frame = PhysicalFrame::containing_address(physical_start_address);

            let end_frame: PhysicalFrame = PhysicalFrame::containing_address(
                physical_start_address + header.file_size() - 1u64,
            );

            let start_page = Page::containing_address(VirtualAddress::new(
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

            // Map data into memory
            if header.file_size() > 0 {
                for frame in PhysicalFrame::range_inclusive(start_frame, end_frame) {
                    let offset = frame - start_frame;
                    // 1:1 mapping
                    let page = start_page + offset;
                    /*
                    println!(
                        "Map: {:x} -> {:x}",
                        frame.address.as_u64(),
                        page.address.as_u64()
                    );
                    */
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
                // .bss that is partially included in ELF (has an actual size) ?
                // never seen it
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
        //println!("Load called at {:#x}, flags = {}", base, flags);
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
