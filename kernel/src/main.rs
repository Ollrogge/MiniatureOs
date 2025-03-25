#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
use api::{BootInfo, PhysicalMemoryRegions};
use core::{alloc::Layout, arch::asm, mem::size_of, panic::PanicInfo};
use kernel::{
    housekeeping_threads::{spawn_finalizer_thread, spawn_idle_thread},
    kernel_init,
    multitasking::{
        process,
        thread::{leave_thread, ThreadPriority},
    },
    print, println, serial_println,
    time::Time,
};
use x86_64::{
    instructions::{hlt, int3},
    memory::MemoryRegion,
};

extern crate alloc;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    serial_println!("Kernel PANIC: {}", info);
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    start(info);
}

fn print_memory_map(map: &PhysicalMemoryRegions) {
    for region in map.iter() {
        serial_println!(
            "Memory region, start: {:#x}, length: {:#x}, usable: {}",
            region.start,
            region.size,
            region.is_usable()
        );
    }
}

fn trigger_int3() {
    int3();
}

#[allow(dead_code)]
fn trigger_invalid_opcode() {
    unsafe {
        asm!("ud2");
    }
}

#[allow(dead_code)]
fn trigger_divide_by_zero() {
    unsafe {
        asm!("mov rax, {0:r}", "mov rcx, {1:r}", "div rcx", in(reg) 4, in(reg) 0);
    }
}

// should cause a pagefault because guard page is hit
// to trigger double fault: unregister page fault handler. Then a page fault
// will be raised which triggers a double fault since the descriptor is invalid
#[allow(dead_code)]
fn stack_overflow() {
    stack_overflow()
}

// using *mut u64 here causes an infinite loop since address is not 8 byte aligned
// todo: this is weird ?, can cause infinite loops at other places ?
#[allow(dead_code)]
fn trigger_page_fault() {
    unsafe { *(0xdeabeef as *mut u8) = 42 };
}

fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}

fn start(info: &'static BootInfo) -> ! {
    println!("Kernel enter");

    print_memory_map(&info.memory_regions);

    let _ = kernel_init(info).expect("Error while trying to initialize kernel");
    println!("Kernel initialized");

    //unsafe { test_buddy_allocator() };
    //println!("Buddy allocator tested");

    //test_heap_allocations();
    //println!("Heap tested");

    trigger_int3();

    process::init(info).unwrap();
    println!("Processes initialized, spawning idle thread");

    spawn_idle_thread().unwrap();
    spawn_finalizer_thread().unwrap();

    let start = Time::now();
    serial_println!("Try time: 5 secs: {}", Time::elapsed_s(start));
    while Time::elapsed_s(start) < 5 {}
    serial_println!("5 secs done");

    // done initializing, kill colonel thread
    leave_thread();

    //trigger_page_fault();
    //stack_overflow();
}
