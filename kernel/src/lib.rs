#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
use api::BootInfo;
extern crate alloc;
use core::iter::Copied;
use x86_64::{
    memory::{Address, MemoryRegion, PhysicalMemoryRegion},
    paging::{
        bump_frame_allocator::BumpFrameAllocator,
        linked_list_frame_allocator::LinkedListFrameAllocator,
        offset_page_table::{OffsetPageTable, PhysicalOffset},
    },
};

pub mod allocator;
pub mod interrupts;
pub mod multitasking;
pub mod paging;
pub mod qemu;
pub mod serial;
pub mod vga;

use allocator::init_heap;

pub fn kernel_init(
    boot_info: &'static BootInfo,
) -> Result<(LinkedListFrameAllocator, OffsetPageTable<PhysicalOffset>), ()> {
    println!("Initializing kernel");
    interrupts::init();

    let pml4t = unsafe { paging::init(boot_info) };

    let pt_offset = PhysicalOffset::new(boot_info.physical_memory_offset);
    let mut page_table = OffsetPageTable::new(pml4t, pt_offset);

    let mut frame_allocator = LinkedListFrameAllocator::new(
        boot_info.memory_regions.iter().copied(),
        boot_info.physical_memory_offset,
    );

    init_heap(&mut page_table, &mut frame_allocator);

    Ok((frame_allocator, page_table))
}
