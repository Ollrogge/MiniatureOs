//! This module contains the stage4 code of the bootloader.
//! So close to kernel now :P
#![no_std]
#![no_main]
use core::{arch::asm, panic::PanicInfo, ptr, slice};
mod elf;
use crate::elf::KernelLoader;
use api::{BootInfo, PhysicalMemoryRegions};
use common::{hlt, BiosInfo, E820MemoryRegion};
use core::alloc::Layout;
use x86_64::{
    frame_allocator::{BumpFrameAllocator, FrameAllocator},
    gdt::{self, SegmentDescriptor},
    memory::{
        Address, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame,
        PhysicalMemoryRegion, VirtualAddress, KIB,
    },
    paging::{FourLevelPageTable, Mapper, PageTable, PageTableEntryFlags},
    println,
    register::{Cr0, Cr0Flags, Efer, EferFlags},
};

// hardcoded for now
const KERNEL_VIRTUAL_BASE: u64 = 0xffffffff80000000;
const KERNEL_STACK_TOP: u64 = 0xffffffff00000000;
const KERNEL_STACK_SIZE: usize = 2 * KIB;

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

fn allocate_and_map_stack<A, M, S>(frame_allocator: &mut A, page_table: &mut M) -> VirtualAddress
where
    A: FrameAllocator<S>,
    M: Mapper<S>,
    S: PageSize,
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
            .expect("Failed to map stack");
    }

    end_page.address
}

// identity-map context switch function, so that we don't get an immediate pagefault
// after switching the active page table
fn identity_map_context_switch_function<A, M, S>(frame_allocator: &mut A, page_table: &mut M)
where
    A: FrameAllocator<S>,
    M: Mapper<S>,
    S: PageSize,
{
    let context_switch_function =
        PhysicalFrame::containing_address(PhysicalAddress::new(context_switch as *const () as u64));
    let flags = PageTableEntryFlags::PRESENT;
    page_table
        .identity_map(context_switch_function, flags, frame_allocator)
        .expect("Identify mapping failed");
}

fn initialize_and_map_gdt<A, M, S>(frame_allocator: &mut A, page_table: &mut M)
where
    A: FrameAllocator<S>,
    M: Mapper<S>,
    S: PageSize,
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

    page_table
        .identity_map(frame, PageTableEntryFlags::PRESENT, frame_allocator)
        .expect("Identity mapping gdt failed");
}

fn allocate_and_map_boot_info<A, M, S>(
    frame_allocator: &mut A,
    page_table: &mut M,
    info: &BiosInfo,
    memory_map: &[E820MemoryRegion],
) -> VirtualAddress
where
    A: FrameAllocator<S>,
    M: Mapper<S>,
    S: PageSize,
{
    let mut boot_info_layout = Layout::new::<BootInfo>();
    let usable_memory_regions_amount = memory_map.iter().filter(|r| r.is_usable()).count();
    println!(
        "Usable memory regions amount: {}",
        usable_memory_regions_amount
    );
    let memory_regions_layout =
        Layout::array::<PhysicalMemoryRegion>(usable_memory_regions_amount).unwrap();
    let (combined_layout, memory_regions_offset) =
        boot_info_layout.extend(memory_regions_layout).unwrap();

    // if this happens need a better allocator which can allocate > 1 frame
    // and ensure they are contiguous
    assert!(
        combined_layout.size() <= S::SIZE.try_into().unwrap(),
        "Required memory for boot info is bigger than page size"
    );

    let frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate frame for boot info");

    // write memory regions information to allocated frame
    let memory_regions_ptr: *mut PhysicalMemoryRegion =
        (frame.address + memory_regions_offset).as_mut_ptr();

    for (idx, mem_region) in memory_map.iter().filter(|r| r.is_usable()).enumerate() {
        let mem_region: PhysicalMemoryRegion = mem_region.into();
        let ptr = unsafe { memory_regions_ptr.add(idx) };
        unsafe { ptr::write(ptr, mem_region) }
    }

    // write bootinfo to allocated frame
    let memory_regions =
        PhysicalMemoryRegions::new(memory_regions_ptr, usable_memory_regions_amount);
    let boot_info = BootInfo::new(info.kernel, info.framebuffer, memory_regions);
    unsafe { ptr::write(frame.address.as_mut_ptr(), boot_info) };

    let virtual_address = VirtualAddress::new(frame.address.as_u64());
    let page = Page::for_address(virtual_address);

    page_table
        .map_to(frame, page, PageTableEntryFlags::PRESENT, frame_allocator)
        .expect("Failed to map boot info");

    virtual_address
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

fn start(info: &BiosInfo) -> ! {
    println!("Stage4");

    enable_nxe_bit();
    enable_write_protect_bit();

    let memory_map: &[E820MemoryRegion] = unsafe {
        slice::from_raw_parts(
            info.memory_map_address as *const _,
            info.memory_map_size.try_into().unwrap(),
        )
    };

    // +1 to get the next frame after the last frame we allocate data in
    let next_free_frame =
        PhysicalFrame::containing_address(PhysicalAddress::new(info.last_physical_address)) + 1;

    let mut allocator =
        BumpFrameAllocator::new_starting_at(next_free_frame, memory_map.iter().copied().peekable());

    let frame = allocator
        .allocate_frame()
        .expect("Failed to allocate frame");

    // 1:1 mapping, therefore frame address = virtual address
    let kernel_page_table_address = VirtualAddress::new(frame.address.as_u64());
    let kernel_page_table = PageTable::initialize_empty_at_address(kernel_page_table_address);
    let mut page_table = FourLevelPageTable::new(kernel_page_table);

    let mut loader = KernelLoader::new(KERNEL_VIRTUAL_BASE, info, &mut page_table, &mut allocator);
    let kernel_entry_point = loader.load_kernel(info);

    let stack_top = allocate_and_map_stack(&mut allocator, &mut page_table);

    identity_map_context_switch_function(&mut allocator, &mut page_table);

    let boot_info_address =
        allocate_and_map_boot_info(&mut allocator, &mut page_table, &info, memory_map);

    initialize_and_map_gdt(&mut allocator, &mut page_table);

    // todo: detect RSDP (Root System Description Pointer)

    println!(
        "Switching to kernel entry point at {:#x}",
        kernel_entry_point.as_u64()
    );

    context_switch(
        kernel_page_table_address.as_u64(),
        stack_top.as_u64(),
        kernel_entry_point.as_u64(),
        boot_info_address.as_u64(),
    );
}
