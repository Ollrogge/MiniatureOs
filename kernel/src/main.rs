#![no_std]
#![no_main]
use bootloader_api::BootInfo;
use core::panic::PanicInfo;

static mut TEST: [u8; 0xabc123] = [0; 0xabc123];

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    loop {}
}
