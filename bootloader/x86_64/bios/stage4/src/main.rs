//! This module contains the stage4 code of the bootloader.
//! So close to kernel now :P
#![no_std]
#![no_main]
#![feature(naked_functions)]
use core::{
    arch::asm,
    panic::PanicInfo,
    ptr::{self},
    slice,
};
mod elf;
mod interrupts;
use crate::elf::KernelLoader;
use api::{BootInfo, PhysicalMemoryRegions};
use common::{hlt, println, BiosInfo, E820MemoryRegion};
use core::alloc::Layout;
use x86_64::{
    gdt::{self, SegmentDescriptor},
    memory::{
        Address, FrameAllocator, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame,
        PhysicalMemoryRegion, PhysicalMemoryRegionType, Size2MiB, Size4KiB, VirtualAddress, KIB,
        TIB,
    },
    paging::{
        bump_frame_allocator::BumpFrameAllocator,
        offset_page_table::{OffsetPageTable, PhysicalOffset},
        Mapper, MapperAllSizes, PageTable, PageTableEntryFlags,
    },
    register::{Cr0, Cr0Flags, Efer, EferFlags},
};

// hardcoded for now
const KERNEL_VIRTUAL_BASE: u64 = 0xffffffff80000000;
const KERNEL_STACK_TOP: u64 = 0xffffffff00000000;
const KERNEL_STACK_SIZE: u64 = 128 * KIB;
const VGA_BUFFER_ADDRESS: PhysicalAddress = PhysicalAddress::new(0xb8000);
// map the complete physical address space at this offset in order to enable
// the kernel to easily access the page table
// https://os.phil-opp.com/paging-implementation/#map-at-a-fixed-offset
// map it at an offset of 10 TB
const PHYSICAL_MEMORY_OFFSET: u64 = 10 * TIB;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {
        hlt();
    }
}

// https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &BiosInfo) -> ! {
    start(info);
}

/// Performs the actual context switch.
fn context_switch(page_table: u64, stack_top: u64, entry_point: u64, boot_info: u64) -> ! {
    unsafe {
        asm!(
            "xor rbp, rbp",
            // Writing to cr3, will invalidate the whole tlb so no need to flush
            "mov cr3, {}",
            "mov rsp, {}",
            "push 0",
            "jmp {}",
            in(reg) page_table,
            in(reg) stack_top,
            in(reg) entry_point,
            in("rdi") boot_info,
        );
    }
    unreachable!();
}

fn allocate_and_map_stack<A, M>(frame_allocator: &mut A, page_table: &mut M) -> VirtualAddress
where
    A: FrameAllocator<Size4KiB>,
    M: Mapper<Size4KiB>,
{
    let end_page = Page::containing_address(VirtualAddress::new(KERNEL_STACK_TOP));
    // grows downwards
    let start_page = Page::containing_address(VirtualAddress::new(
        KERNEL_STACK_TOP - KERNEL_STACK_SIZE as u64,
    ));
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate frame for stack");

        let flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::NO_EXECUTE;

        page_table
            .map_to(frame, page, flags, frame_allocator)
            .expect("Failed to map stack page")
            .ignore();
    }

    // catch kernel stack overflows
    let guard_page = Page::containing_address(start_page.address - Size4KiB::SIZE);
    assert!(guard_page != start_page);
    let frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate frame for guard page");

    page_table
        .map_to(
            frame,
            guard_page,
            PageTableEntryFlags::NONE,
            frame_allocator,
        )
        .expect("Failed to map guard page")
        .ignore();

    end_page.address
}

// identity-map context switch function, so that we don't get an immediate pagefault
// after switching the active page table
fn identity_map_context_switch_function<A, M>(frame_allocator: &mut A, page_table: &mut M)
where
    A: FrameAllocator<Size4KiB>,
    M: Mapper<Size4KiB>,
{
    let context_switch_function =
        PhysicalFrame::containing_address(PhysicalAddress::new(context_switch as *const () as u64));
    let flags = PageTableEntryFlags::PRESENT;
    page_table
        .identity_map(context_switch_function, flags, frame_allocator)
        .expect("Identify mapping failed")
        .ignore();
}

fn initialize_and_map_gdt<A, M>(frame_allocator: &mut A, page_table: &mut M)
where
    A: FrameAllocator<Size4KiB>,
    M: MapperAllSizes,
{
    let frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate gdt frame");

    let virtual_address = VirtualAddress::new(frame.address.as_u64());

    let gdt = gdt::GlobalDescriptorTable::initialize_at_address(virtual_address);

    // kinda useless since equal to long mode descriptors.
    gdt.add_entry(SegmentDescriptor::kernel_code_segment());
    gdt.add_entry(SegmentDescriptor::kernel_data_segment());

    gdt.load();

    // we dont need to reset the segment registers, since they still contain the
    // correct indexes. We only exchanged the descriptors.

    // TODO: why is this actually needed ? cpu accesses the gdt based on physical address
    page_table
        .identity_map(frame, PageTableEntryFlags::PRESENT, frame_allocator)
        .expect("Identity mapping gdt failed")
        .ignore();
}

/// Returns the current state of the memory (which regions are used and which are not)
//  Splits a memory region into two of only part of it is used
fn build_memory_map<A, S>(
    allocator: &A,
    regions: &[E820MemoryRegion],
    last_frame: &PhysicalFrame<S>,
) -> [Option<PhysicalMemoryRegion>; 0x20]
where
    A: FrameAllocator<S>,
    S: PageSize,
{
    let mut new_regions = [None; 0x20];
    let mut idx: usize = 0;
    for (i, region) in regions.iter().enumerate() {
        if !region.is_usable() {
            new_regions[idx] = Some(region.into());
            idx += 1;
        } else {
            // MBR & stage1, stage2 region => mark as used
            if region.start() == 0x0 {
                let mut new_region: PhysicalMemoryRegion = region.into();
                new_region.typ = PhysicalMemoryRegionType::Reserved;
                new_regions[idx] = Some(new_region);
                continue;
            }
            // split region into usable and unusable pair if the region is not
            // completely allocated
            if region.contains(last_frame.address.as_u64()) {
                let sz = last_frame.end() - region.start();
                let used_region = PhysicalMemoryRegion::new(
                    region.start(),
                    sz,
                    PhysicalMemoryRegionType::Reserved,
                );

                new_regions[idx] = Some(used_region);
                idx += 1;

                if last_frame.end() != region.end() {
                    let sz = region.end() - last_frame.end();
                    let free_region = PhysicalMemoryRegion::new(
                        last_frame.end(),
                        sz,
                        PhysicalMemoryRegionType::Free,
                    );

                    new_regions[idx] = Some(free_region);
                    idx += 1;
                }
            } else {
                new_regions[idx] = Some(region.into());
                idx += 1;
            }
        }

        // Need a better solution if this happens or increase the array size
        assert!(idx < new_regions.len())
    }

    new_regions
}

fn allocate_and_map_boot_info<A, M>(
    frame_allocator: &mut A,
    page_table: &mut M,
    info: &BiosInfo,
    e820_memory_map: &[E820MemoryRegion],
) -> VirtualAddress
where
    A: FrameAllocator<Size4KiB>,
    M: MapperAllSizes,
{
    let frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate frame for boot info");

    let mut boot_info_layout = Layout::new::<BootInfo>();
    let memory_map = build_memory_map(frame_allocator, e820_memory_map, &frame);
    let usable_memory_regions_amount = memory_map.iter().filter(|r| r.is_some()).count();

    // write MemoryRegions array onto the same frame behind the bootinfo struct
    let memory_regions_layout =
        Layout::array::<PhysicalMemoryRegion>(usable_memory_regions_amount).unwrap();
    let (combined_layout, memory_regions_offset) =
        boot_info_layout.extend(memory_regions_layout).unwrap();

    assert!(
        combined_layout.size() <= Size4KiB::SIZE.try_into().unwrap(),
        "Required memory for boot info is bigger than page size"
    );

    // write memory regions information to allocated frame
    let memory_regions_ptr: *mut PhysicalMemoryRegion =
        (frame.address + memory_regions_offset).as_mut_ptr();

    for (idx, mem_region) in memory_map.iter().filter_map(|r| r.as_ref()).enumerate() {
        let ptr = unsafe { memory_regions_ptr.add(idx) };
        unsafe { ptr::write(ptr, *mem_region) };
    }

    // write bootinfo to allocated frame
    let memory_regions =
        PhysicalMemoryRegions::new(memory_regions_ptr, usable_memory_regions_amount);
    let boot_info = BootInfo::new(
        info.kernel,
        info.framebuffer,
        memory_regions,
        PHYSICAL_MEMORY_OFFSET,
    );
    unsafe { ptr::write(frame.address.as_mut_ptr(), boot_info) };

    let virtual_address = VirtualAddress::new(frame.address.as_u64());
    let page = Page::for_address(virtual_address);

    page_table
        .map_to(frame, page, PageTableEntryFlags::PRESENT, frame_allocator)
        .expect("Failed to map boot info")
        .ignore();

    virtual_address
}

// Map the complete physical address space at an offset into kernel memory space
fn map_complete_physical_memory_space_at_an_offset_into_kernel<A, M>(
    frame_allocator: &mut A,
    page_table: &mut M,
    highest_physical_address: PhysicalAddress,
    offset: VirtualAddress,
) where
    A: FrameAllocator<Size4KiB>,
    M: MapperAllSizes,
{
    println!(
        "Mapping complete physical address space to offset: {:#x}",
        offset.as_u64()
    );
    let start = PhysicalFrame::containing_address(PhysicalAddress::new(0));
    let end = PhysicalFrame::containing_address(highest_physical_address);
    let alignment = Size2MiB::SIZE;
    // check 2MiB alignment
    assert!(offset.as_u64() % alignment == 0);

    for frame in PhysicalFrame::<Size2MiB>::range_inclusive(start, end) {
        let page = Page::containing_address(offset + frame.start());

        let flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::NO_EXECUTE;
        page_table
            .map_to(frame, page, flags, frame_allocator)
            .expect("Failed to map all of RAM to kernel space")
            .ignore();
    }
}

/// Enable the No execute enable bit in the Efer register
/// Allows to set the Execute Disable flag on page table entries
fn enable_nxe_bit() {
    unsafe {
        Efer::update(|val| *val |= EferFlags::NO_EXECUTE_ENABLE);
    }
}

// Make the kernel respect the write-protection bits even when in ring 0 by default
fn enable_write_protect_bit() {
    unsafe {
        Cr0::update(|val| *val |= Cr0Flags::WRITE_PROTECT);
    }
}
fn trigger_page_fault() {
    unsafe { *(0xdeabeefdead as *mut u8) = 42 };
}

fn start(info: &BiosInfo) -> ! {
    println!("Stage4");

    interrupts::init();

    enable_nxe_bit();
    enable_write_protect_bit();

    let memory_map: &[E820MemoryRegion] = unsafe {
        slice::from_raw_parts(
            info.memory_map_address as *const _,
            info.memory_map_size.try_into().unwrap(),
        )
    };

    // +1 to get the next frame after the last frame we allocated data in
    let next_free_frame =
        PhysicalFrame::containing_address(PhysicalAddress::new(info.last_physical_address)) + 1;

    let mut allocator =
        BumpFrameAllocator::new_starting_at(next_free_frame, memory_map.iter().copied().peekable());

    let kernel_page_table_frame = allocator
        .allocate_frame()
        .expect("Failed to allocate frame for kernel page table");

    let kernel_page_table_address = VirtualAddress::new(kernel_page_table_frame.start());
    let kernel_page_table =
        unsafe { PageTable::initialize_empty_at_address(kernel_page_table_address) };
    // 1:1 mapping
    let mapping = PhysicalOffset::new(0);
    let mut page_table = OffsetPageTable::new(kernel_page_table, mapping);

    let mut loader = KernelLoader::new(KERNEL_VIRTUAL_BASE, info, &mut page_table, &mut allocator);
    let kernel_entry_point = loader.load_kernel(info);

    let stack_top = allocate_and_map_stack(&mut allocator, &mut page_table);

    identity_map_context_switch_function(&mut allocator, &mut page_table);

    initialize_and_map_gdt(&mut allocator, &mut page_table);

    // No more allocations should be done after the boot info has been allocated.
    // Otherwise memory regions information is incorrect
    let boot_info_address =
        allocate_and_map_boot_info(&mut allocator, &mut page_table, &info, memory_map);

    let max_physical_address = allocator.max_physical_address();

    // identity map vga_buffer
    page_table
        .identity_map(
            PhysicalFrame::<Size4KiB>::containing_address(VGA_BUFFER_ADDRESS),
            PageTableEntryFlags::PRESENT | PageTableEntryFlags::WRITABLE,
            &mut allocator,
        )
        .expect("Failed to idenity map VGA buffer")
        .ignore();

    map_complete_physical_memory_space_at_an_offset_into_kernel(
        &mut allocator,
        &mut page_table,
        max_physical_address,
        VirtualAddress::new(PHYSICAL_MEMORY_OFFSET),
    );

    // todo: detect RSDP (Root System Description Pointer)
    println!(
        "Switching to kernel entry point at {:#x}, kernel page table at address: {:#x}",
        kernel_entry_point.as_u64(),
        kernel_page_table_frame.start()
    );

    context_switch(
        kernel_page_table_frame.start(),
        stack_top.as_u64(),
        kernel_entry_point.as_u64(),
        boot_info_address.as_u64(),
    );
}
