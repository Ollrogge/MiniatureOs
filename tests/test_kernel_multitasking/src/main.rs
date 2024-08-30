#![no_std]
#![no_main]
use api::BootInfo;
use core::panic::PanicInfo;
use kernel::{
    error::KernelError,
    housekeeping_threads, kernel_init,
    memory::manager::AllocationStrategy,
    multitasking::{
        process::{self, ThreadId},
        scheduler::Scheduler,
        thread::{leave_thread, ThreadPriority},
    },
    qemu, serial_println,
    time::Time,
};

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
    serial_println!("Test kernel enter");

    kernel_init(info).expect("Kernel initialization failed");

    process::init(info).expect("Initializing processes failed");

    housekeeping_threads::spawn_finalizer_thread().expect("Failed to spawn finializer thread");

    let start = Time::now();
    while Time::elapsed_s(start) < 1 {}

    qemu::exit(qemu::QemuExitCode::Success);
}
