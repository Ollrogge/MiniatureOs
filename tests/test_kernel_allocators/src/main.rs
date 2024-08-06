#![no_std]
#![no_main]
use api::BootInfo;
use core::panic::PanicInfo;
use kernel::{kernel_init, println, qemu};

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

    test_heap_allocations();

    qemu::exit(qemu::QemuExitCode::Success);
}
