#![no_std]
#![no_main]
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    loop {}
}
