#![no_std]
#![no_main]
use common::{hlt, println, BiosInfo};
use core::panic::PanicInfo;

// Can not use any BIOS functions anymore since we are in protected mode.

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &BiosInfo) -> ! {
    start(info);
}

fn start(info: &BiosInfo) -> ! {
    loop {
        hlt();
    }
}
