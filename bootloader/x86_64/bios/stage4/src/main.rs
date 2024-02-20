//! Stage4 of the bootloader. Long mode
//! So close to kernel now :P
#![no_std]
#![no_main]
use core::{arch::asm, panic::PanicInfo, ptr, slice};
mod elf;
use common::{hlt, BiosInfo, E820MemoryRegion};
use x86_64::{
    frame_allocator::{BumpFrameAllocator, FrameAllocator},
    memory::{
        Address, KiB, MemoryRegion, Page, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB,
        VirtualAddress,
    },
    paging::{FourLevelPageTable, Mapper, PageTable, PageTableEntryFlags},
    println,
    register::{Cr0, Cr0Flags, Efer, EferFlags},
};

use crate::elf::KernelLoader;

// hardcoded for now;
const KERNEL_VIRTUAL_BASE: u64 = 0xffffffff80000000;
const KERNEL_STACK_TOP: u64 = 0xffffffff00000000;
const KERNEL_STACK_SIZE: usize = 2 * KiB;

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
unsafe fn context_switch(page_table: u64, stack_top: u64, entry_point: u64, boot_info: u64) -> ! {
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
    let start_page = Page::containing_address(VirtualAddress::new(KERNEL_STACK_TOP));
    let end_page = Page::containing_address(VirtualAddress::new(
        KERNEL_STACK_TOP + KERNEL_STACK_SIZE as u64,
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

    start_page.address
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

/// Enable the No execute enable bit in the Efer register
/// Allows to set the Execute Disable flag on page table entries
fn enable_nxe_bit() {
    Efer::update(|val| *val |= EferFlags::NO_EXECUTE_ENABLE);
}

// Make the kernel respect the write-protection bits even when in ring 0 by default
fn enable_write_protect_bit() {
    Cr0::update(|val| *val |= Cr0Flags::WRITE_PROTECT);
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
    let kernel_pml4t_address = VirtualAddress::new(frame.address.as_u64());

    let mut kernel_page_table = PageTable::initialize_empty_at_address(kernel_pml4t_address);
    let kernel_page_table = unsafe { &mut *kernel_page_table };

    let mut page_table = FourLevelPageTable::new(kernel_page_table);

    let mut loader = KernelLoader::new(KERNEL_VIRTUAL_BASE, info, &mut page_table, &mut allocator);

    let kernel_entry_point = loader.load_kernel(info);

    let stack_top = allocate_and_map_stack(&mut allocator, &mut page_table);

    identity_map_context_switch_function(&mut allocator, &mut page_table);
    println!("test3");

    loop {
        hlt();
    }
}
