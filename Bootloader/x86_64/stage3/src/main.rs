#![no_std]
#![no_main]
use common::{hlt, println, BiosInfo};
use core::panic::PanicInfo;

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &BiosInfo) -> ! {
    start(info);
}

fn start(info: &BiosInfo) -> ! {
    println!("Stage3 ");

    loop {
        hlt();
    }
}
