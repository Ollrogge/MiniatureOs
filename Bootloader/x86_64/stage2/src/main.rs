#![no_std]
#![no_main]

use util::{panic, print};

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(disk_number: u8, partition_table_start: *const u8) -> ! {
    start(disk_number, partition_table_start)
}

fn start(disk_number: u8, partition_table_start: *const u8) -> ! {
    print("\rStage2\n");

    loop {}
}
