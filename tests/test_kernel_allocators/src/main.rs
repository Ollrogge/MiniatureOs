#![no_std]
#![no_main]
use api::BootInfo;
use core::{alloc::Layout, panic::PanicInfo};
use kernel::{allocator::ALLOCATOR, kernel_init, println, qemu};

extern crate alloc;
use alloc::{boxed::Box, vec::Vec};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    start(info);
}

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

    //assert!(u64::min(c1.as_ref().start(), c2.as_ref().start()) == addr);

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

    //assert!(c4.as_ref().start() == addr);
    //assert!(c4.as_ref().start() == addr);

    allocator.dealloc(c4);
}

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

fn start(info: &'static BootInfo) -> ! {
    kernel_init(info).unwrap();
    println!("Test kernel initialized, running tests");

    unsafe { test_buddy_allocator() };

    test_heap_allocations();

    qemu::exit(qemu::QemuExitCode::Success);
}
