//! Stage4 of the bootloader. Long mode
//! So close to kernel now :P
#![no_std]
#![no_main]
use core::{panic::PanicInfo, ptr};
mod elf;
mod print;
use common::{hlt, BiosInfo};
use x86_64::{
    frame_allocator::{BumpFrameAllocator, FrameAllocator},
    memory::{Address, PhysicalAddress, PhysicalFrame, Size4KiB, VirtualAddress},
    paging::PageTable,
};

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

/*
 + frame allocator
 + load load all kernel sections into memory

*/

fn start(info: &BiosInfo) -> ! {
    println!("Stage4");

    let mut allocator = BumpFrameAllocator::new_starting_at(
        PhysicalFrame::at_address(PhysicalAddress::new(info.last_physical_address)),
        info.memory_map.iter().copied().peekable(),
    );

    let frame = allocator
        .allocate_frame()
        .expect("Failed to allocate frame");

    // 1:1 mapping
    let kernel_pml4t_address = VirtualAddress::new(frame.address.as_u64());

    let mut kernel_page_table = PageTable::initialize_empty_at_address(kernel_pml4t_address);
    let kernel_page_table = unsafe { &mut *kernel_page_table };

    elf::map_kernel(kernel_page_table, info, &mut allocator);

    // map the kernel and all its sections, do relocations

    loop {
        hlt();
    }
}
