#![no_std]
#![no_main]

use core::{arch::asm, arch::global_asm, panic::PanicInfo, slice, usize};
mod mbr;
use mbr::PartitionTableEntry;
use util::{print_char, DiskAddressPacket};

global_asm!(include_str!("boot.asm"));

extern "C" {
    static _partition_table: u8;
    static _second_stage_start: u8;
}

unsafe fn partition_table_raw() -> *const u8 {
    unsafe { &_partition_table }
}

fn second_stage_start() -> u32 {
    let ptr: *const u8 = unsafe { &_second_stage_start };
    ptr as u32
}

// local def because print in util is too big (overflows mbr code sec)
fn print(s: &[u8]) {
    for &c in s.iter() {
        print_char(c);
    }
}

#[no_mangle]
pub extern "C" fn first_stage(disk_number: u8) {
    print(b"Stage1\0");

    let partition_table = unsafe { slice::from_raw_parts(partition_table_raw(), 16 * 4) };
    let pte = mbr::get_partition(partition_table, 0);

    const SECTOR_SIZE: usize = 512;

    let mut start_lba: u64 = pte.logical_block_address.into();
    let mut sector_count = pte.sector_count;
    let mut buffer_address = second_stage_start();

    while sector_count > 0 {
        let sectors = u32::min(sector_count, 127) as u16;
        let packet = DiskAddressPacket::new(buffer_address, sectors, start_lba);

        unsafe {
            packet.load(disk_number);
        }

        sector_count -= u32::from(sectors);
        start_lba += u64::from(sectors);
        buffer_address += u32::from(sectors) * SECTOR_SIZE as u32;
    }

    loop {}
}
