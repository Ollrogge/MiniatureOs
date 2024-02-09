//! Stage4 of the bootloader. Long mode
//! So close to kernel now :P
#![no_std]
#![no_main]
use core::panic::PanicInfo;
mod print;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {
        hlt();
    }
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &BiosInfo) -> ! {
    start(info);
}

fn start(info: &BiosInfo) -> ! {
    println!("\nStage4");
}
