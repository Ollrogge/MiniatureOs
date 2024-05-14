#![no_std]
#![no_main]
#![feature(naked_functions)]
use api::BootInfo;
extern crate alloc;
use x86_64::println;

pub mod interrupts;
pub mod memory;
pub mod qemu;

pub fn kernel_init(boot_info: &'static BootInfo) -> Result<(), ()> {
    println!("Initializing kernel");
    interrupts::init();
    Ok(())
}
