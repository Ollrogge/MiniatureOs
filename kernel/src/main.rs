#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
use alloc::string::String;
use api::{BootInfo, PhysicalMemoryRegions};
use core::{alloc::Layout, arch::asm, mem::size_of, panic::PanicInfo};
use kernel::{
    allocator::ALLOCATOR,
    housekeeping_threads::{spawn_finalizer_thread, spawn_idle_thread},
    kernel_init,
    multitasking::{
        process,
        thread::{leave_thread, ThreadPriority},
    },
    print, println, serial_println,
};
use x86_64::{
    instructions::{hlt, int3},
    memory::MemoryRegion,
};

extern crate alloc;
use alloc::{boxed::Box, vec::Vec};

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

// TODO: put this into the test_kernel
#[allow(dead_code)]
unsafe fn test_buddy_allocator() {
    let mut allocator = ALLOCATOR.lock();
    let layout_x100 = Layout::from_size_align(0x100, size_of::<usize>()).unwrap();
    let layout_x200 = Layout::from_size_align(0x200, size_of::<usize>()).unwrap();
    let layout_x400 = Layout::from_size_align(0x400, size_of::<usize>()).unwrap();

    // Test easy merge
    let c1 = allocator.alloc(layout_x100).unwrap();
    let c2 = allocator.alloc(layout_x100).unwrap();

    let addr = u64::min(c1.as_ref().start(), c2.as_ref().start());

    // c1 and c2 should be merged into 1 0x200 sized chunk
    allocator.dealloc(c1);
    allocator.dealloc(c2);

    let c3 = allocator.alloc(layout_x200).unwrap();
    assert!(c3.as_ref().start() == addr);

    let addr = c3.as_ref().start();
    allocator.dealloc(c3);

    // Test multistage merge

    // c1 and c2 should be created from the c3 we just deallocated
    let c1 = allocator.alloc(layout_x100).unwrap();
    let c2 = allocator.alloc(layout_x100).unwrap();

    assert!(u64::min(c1.as_ref().start(), c2.as_ref().start()) == addr);

    let c3 = allocator.alloc(layout_x200).unwrap();
    println!(
        "C3 address: {:#x}, min address before: {:#x}",
        c3.as_ref().start(),
        addr
    );
    let addr = u64::min(
        c3.as_ref().start(),
        u64::min(c1.as_ref().start(), c2.as_ref().start()),
    );
    // merge 2* 0x100 into 0x200
    allocator.dealloc(c1);
    allocator.dealloc(c2);
    // free c3 causing it to be merged with the 0x200 chunk created by
    // deallocating c1 and c2. Should create 1 0x400 sized chunk
    allocator.dealloc(c3);

    let c4 = allocator.alloc(layout_x400).unwrap();

    assert!(c4.as_ref().start() == addr);
    assert!(c4.as_ref().start() == addr);

    allocator.dealloc(c4);
}

#[allow(dead_code)]
fn test_heap_allocations() {
    {
        let heap_value_1 = Box::new(41);
        let heap_value_2 = Box::new(13);
        assert_eq!(*heap_value_1, 41);
        assert_eq!(*heap_value_2, 13);

        let n = 4000;
        let mut vec = Vec::new();
        for i in 0..n {
            vec.push(i);
        }
        assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
    }

    // test for any race conditions between timer and this loop
    for i in 0..100000 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}

fn start(info: &'static BootInfo) -> ! {
    println!("Hello from kernel");

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

    // done initializing, kill colonel thread
    leave_thread();

    //trigger_page_fault();
    //stack_overflow();
}
