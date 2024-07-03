#![no_std]
#![no_main]
use api::BootInfo;
use core::panic::PanicInfo;
use kernel::{kernel_init, println, qemu};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    start(info);
}

fn start(info: &'static BootInfo) -> ! {
    kernel_init(info).unwrap();
    println!("Hello from test kernel");

    qemu::exit(qemu::QemuExitCode::Success);
}
