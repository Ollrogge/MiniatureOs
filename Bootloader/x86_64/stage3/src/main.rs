//! Stage3 of the bootloader. Protected mode
//! Cant use BIOS functions anymore so need a small UART driver for text output
#![no_std]
#![no_main]
use common::{gdt::*, hlt, BiosInfo};
use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use lazy_static::lazy_static;

mod mutex;
mod paging;
mod print;

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(SegmentDescriptor::long_mode_code_segment());
        gdt.add_entry(SegmentDescriptor::long_mode_data_segment());
        gdt
    };
}

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
    GDT.clear_interrupts_and_load();
    loop {
        hlt();
    }
}
