#![no_std]
#![no_main]
use api::{BootInfo, PhysicalMemoryRegions};
use core::{arch::asm, panic::PanicInfo};
use x86_64::{
    instructions::int3,
    memory::{MemoryRegion, PhysicalMemoryRegion},
    println,
    register::Cr0,
};
mod interrupts;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    start(info);
}

fn print_memory_map(map: &PhysicalMemoryRegions) {
    for region in map.iter() {
        println!(
            "Memory region, start: {:#x}, length: {:#x} ",
            region.start, region.size
        );
    }
}

fn start(info: &'static BootInfo) -> ! {
    println!("Hello from kernel <3");

    print_memory_map(&info.memory_regions);

    interrupts::init();
    println!("Interrupts initialized");

    int3();

    loop {}
}
