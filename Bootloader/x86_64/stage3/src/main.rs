#![no_std]
#![no_main]
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(disk_number: u8, partition_table_start: *const u8) -> ! {
    loop {}
}
