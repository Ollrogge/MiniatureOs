#![no_std]
#![no_main]
#![feature(naked_functions)]
use api::BootInfo;
extern crate alloc;
use x86_64::{
    paging::offset_page_table::{OffsetPageTable, PhysicalOffset},
    println,
};

pub mod interrupts;
pub mod memory;
pub mod paging;
pub mod qemu;

pub fn kernel_init(boot_info: &'static BootInfo) -> Result<(), ()> {
    println!("Initializing kernel");
    interrupts::init();

    let pml4t = unsafe { paging::init(boot_info) };

    let mapping = PhysicalOffset::new(boot_info.physical_memory_offset);

    let page_table = OffsetPageTable::new(pml4t, mapping);
    Ok(())
}
