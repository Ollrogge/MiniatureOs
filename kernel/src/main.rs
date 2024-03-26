#![no_std]
#![no_main]
use api::{BootInfo, PhysicalMemoryRegions};
use core::{arch::asm, panic::PanicInfo};
use x86_64::{
    memory::{MemoryRegion, PhysicalMemoryRegion},
    println,
    register::Cr0,
};

static mut TEST: [u8; 0xabc123] = [0; 0xabc123];

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

    loop {}
}
