//! Stage3 of the bootloader. Protected mode
//! Cant use BIOS functions anymore so need a small UART driver for text output
#![no_std]
#![no_main]
use common::{hlt, BiosInfo};
use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::write_volatile;

mod mutex;
mod paging;
mod print;
mod uart;

// Can not use any BIOS functions anymore since we are in protected mode.

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
    println!("\nStage3");
    paging::init();
    println!("Paging enabled and I am alive ?");
    loop {
        hlt();
    }
}
