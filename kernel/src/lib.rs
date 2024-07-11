#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
use api::BootInfo;
use error::KernelError;
extern crate alloc;
use core::{iter::Copied, ops::DerefMut};
use x86_64::{
    memory::{Address, MemoryRegion, PhysicalMemoryRegion},
    paging::{
        bump_frame_allocator::BumpFrameAllocator,
        linked_list_frame_allocator::LinkedListFrameAllocator,
        offset_page_table::{OffsetPageTable, PhysicalOffset},
    },
};

pub mod allocator;
pub mod error;
pub mod interrupts;
pub mod memory;
pub mod multitasking;
pub mod paging;
pub mod qemu;
pub mod serial;
pub mod vga;

use memory::manager::MemoryManager;

pub fn kernel_init(boot_info: &'static BootInfo) -> Result<(), KernelError> {
    println!("Initializing kernel");

    MemoryManager::the().lock().init(boot_info)?;

    interrupts::init();

    Ok(())
}
