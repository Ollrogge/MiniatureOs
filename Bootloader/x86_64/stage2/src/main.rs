#![no_std]
#![no_main]
use common::mbr;
use core::{arch::asm, slice};
use lazy_static::lazy_static;

use common::{
    gdt::{GlobalDescriptorTable, SegmentDescriptor},
    panic, print,
};

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(SegmentDescriptor::protected_mode_code_segment());
        gdt.add_entry(SegmentDescriptor::protected_mode_data_segment());
        gdt
    };
}

fn enter_unreal_mode() {
    let ds: u16;
    let ss: u16;

    unsafe {
        asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
    }

    GDT.clear_interrupts_and_load();
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(disk_number: u8, partition_table_start: *const u8) -> ! {
    start(disk_number, partition_table_start)
}

fn start(disk_number: u8, partition_table_start: *const u8) -> ! {
    enter_unreal_mode();

    print("\rStage2 \n");

    let partition_table = unsafe { slice::from_raw_parts(partition_table_start, 4 * 16) };

    loop {}
}
