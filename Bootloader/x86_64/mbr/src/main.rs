#![no_std]
#![no_main]

use core::{arch::asm, arch::global_asm, panic::PanicInfo, slice, usize};
global_asm!(include_str!("boot.asm"));
mod mbr;
use mbr::PartitionTableEntry;
use util::{fail, print, DiskAddressPacket};

extern "C" {
    static _partition_table: u8;
    static _second_stage_start: u8;
}

unsafe fn partition_table_raw() -> *const u8 {
    unsafe { &_partition_table }
}

fn second_stage_start() -> u32 {
    let ptr: *const u8 = unsafe { &second_stage_start };
    ptr as u32
}

/*

        unsafe {
            asm!(
                "push 0x7a", // error code `z`, passed to `fail` on error
                "mov {1:x}, si", // backup the `si` register, whose contents are required by LLVM
                "mov si, {0:x}",
                "int 0x13",
                "jc fail",
                "pop si", // remove error code again
                "mov si, {1:x}", // restore the `si` register to its prior state
                in(reg) self_addr,
                out(reg) _,
                in("ax") 0x4200u16,
                in("dx") disk_number,
            );
        }
*/

unsafe fn load_partition(pte: PartitionTableEntry) {
    unsafe { asm!("push 'h'", "mov si, {0:x}", "int 0x13", "jc fail",) }
}

#[no_mangle]
pub extern "C" fn first_stage(disk_number: u8) {
    print("Stage1");

    let partition_table = unsafe { slice::from_raw_parts(partition_table_raw(), 16 * 4) };

    let pte = mbr::get_partition(partition_table, 0);

    const SECTOR_SIZE: usize = 512;

    let mut start_lba: u64 = pte.logical_block_address.into();
    let mut sector_count = pte.sector_count;
    let mut buffer_address = second_stage_start();

    while (sector_count > 0) {
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
