#![no_std]
#![no_main]
use core::arch::asm;
use core::panic::PanicInfo;

pub mod dap;
pub mod disk;
pub mod fat;
pub mod gdt;
pub mod mbr;
pub mod print;

pub fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        hlt();
    }
}

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    panic!("Fail called with code: {:x}", code);
}
