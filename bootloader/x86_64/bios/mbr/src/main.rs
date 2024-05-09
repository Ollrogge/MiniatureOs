#![no_std]
#![no_main]
//! This module contains the master boot record code (stage1). This code executes in real mode.
//! There is only 446 bytes space for the code so not much can be done.
//! Tasks:
//!     - Load second stage into memory and jump to it
//!
//! Some Notes on real mode:
//!     - NOT 16 bit. 32 bit registers are accessible (Operand Size Override Prefix)
//!     - real = all addresses correspond to real locations in memory
//!     - access to BIOS functions
//!     - Segment registers = contents interpreted as the most significant 16
//!     bits of a linear 20-bit address
//!     - no support for memory protection, code privilege levels, paging, multitasking
//!     - segment registers hold direct segment addresses.
//!     - Segmentation: Addressing works based on segment selectors + an offset.
//!     Physical address = segment * 16 + offset. Since both segment and offset
//!     are 16 bit values, the maximum addressable address is 1MiB.
//!     (actually max is 0xFFFF×16 + 0xFFFF=0x10FFEF, which is 1,114,095), but
//!     addresses wrap around so 1MiB is limit

use core::{arch::global_asm, slice, usize};

mod dap;
mod mbr;
mod util;

use util::{fail, print};

global_asm!(include_str!("boot.asm"));

extern "C" {
    static _partition_table: u8;
    static _second_stage_start: u8;
}

unsafe fn partition_table_raw() -> *const u8 {
    unsafe { &_partition_table }
}

/// Get second stage address based on symbol exported by linker script
fn second_stage_start() -> u32 {
    let ptr: *const u8 = unsafe { &_second_stage_start };
    ptr as u32
}

#[no_mangle]
pub extern "C" fn first_stage(disk_index: u16) {
    print(b"Stage1\n\r\0");

    // load the MBR partition table
    let partition_table = unsafe { slice::from_raw_parts(partition_table_raw(), 4 * 16) };
    // get first entry = 2nd stage information
    let pte = mbr::get_partition(partition_table, 0);

    const SECTOR_SIZE: usize = 512;

    let mut start_lba = u64::from(pte.logical_block_address);
    let mut sector_count = pte.sector_count;
    let mut buffer_address = second_stage_start();

    while sector_count > 0 {
        let sectors = u32::min(sector_count, 0x20) as u16;
        let packet = dap::DiskAddressPacket::new(buffer_address, sectors, start_lba);

        unsafe {
            packet.load(disk_index);
        }

        sector_count -= u32::from(sectors);
        start_lba += u64::from(sectors);
        buffer_address += u32::from(sectors) * SECTOR_SIZE as u32;
    }

    let second_stage_entry: extern "C" fn(disk_number: u16, partition_table: *const u8) =
        unsafe { core::mem::transmute(second_stage_start() as *const ()) };

    let partition_table = unsafe { partition_table_raw() };

    second_stage_entry(disk_index, partition_table);

    fail(b'F');
}
