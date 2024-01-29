#![no_std]
#![no_main]

use common::{dap::DiskAddressPacket, fail, mbr::get_partition, print_char};
use core::{arch::global_asm, slice, usize};

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
    print(b"Stage1\n\r\0");

    let partition_table = unsafe { slice::from_raw_parts(partition_table_raw(), 4 * 16) };
    let pte = get_partition(partition_table, 0);

    const SECTOR_SIZE: usize = 512;

    let mut start_lba: u64 = pte.logical_block_address.into();
    let mut sector_count = pte.sector_count;
    let mut buffer_address = second_stage_start();

    while sector_count > 0 {
        let sectors = u32::min(sector_count, 0x20) as u16;
        let packet = DiskAddressPacket::new(buffer_address, sectors, start_lba);

        unsafe {
            packet.load(disk_number);
        }

        sector_count -= u32::from(sectors);
        start_lba += u64::from(sectors);
        buffer_address += u32::from(sectors) * SECTOR_SIZE as u32;
    }

    let second_stage_entry: extern "C" fn(disk_number: u8, partition_table: *const u8) =
        unsafe { core::mem::transmute(second_stage_start() as *const ()) };

    let partition_table = unsafe { partition_table_raw() };

    second_stage_entry(disk_number, partition_table);

    print(b"Failed to start stage2\n\r\0");
    fail(b'F');
}
