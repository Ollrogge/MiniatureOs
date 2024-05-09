#![no_std]
#![no_main]
#![feature(naked_functions)]
extern crate alloc;
use api::{BootInfo, PhysicalMemoryRegions};
use core::{arch::asm, panic::PanicInfo};
use x86_64::{
    instructions::int3,
    memory::{MemoryRegion, PhysicalMemoryRegion},
    println,
    register::Cr0,
};
mod interrupts;
mod memory;

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

fn trigger_int3() {
    int3();
}

fn trigger_invalid_opcode() {
    unsafe {
        asm!("ud2");
    }
}

fn trigger_divide_by_zero() {
    unsafe {
        asm!("mov rax, {0:r}", "mov rcx, {1:r}", "div rcx", in(reg) 4, in(reg) 0);
    }
}

// should cause a pagefault because guard page is hit
fn stack_overflow() {
    stack_overflow()
}

// using *mut u64 here causes an infinite loop since address is not 8 byte aligned
// todo: this is weird ?, can cause infinite loops at other places ?
fn trigger_page_fault() {
    unsafe { *(0xdeabeef as *mut u8) = 42 };
}

fn start(info: &'static BootInfo) -> ! {
    println!("Hello from kernel <3");

    print_memory_map(&info.memory_regions);

    interrupts::init();
    println!("Interrupts initialized");

    // invalid opcode
    /*
     */
    trigger_int3();
    trigger_page_fault();

    println!("Did not crash, successfully returned from int3");

    //stack_overflow();

    loop {}
}
