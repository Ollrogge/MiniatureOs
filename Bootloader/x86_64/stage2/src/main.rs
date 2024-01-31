#![no_std]
#![no_main]
use common::mbr::{get_partition, PartitionTableEntry};
use common::{disk, fat};
use common::{hlt, println};
use core::any::Any;
use core::{arch::asm, slice};
use lazy_static::lazy_static;

// 1 MiB
const STAGE3_DST: *mut u8 = 0x0010_0000 as *mut u8;

use common::{
    gdt::{GlobalDescriptorTable, SegmentDescriptor},
    print,
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

    println!("Stage2 \r\n");

    let partition_table_raw = unsafe { slice::from_raw_parts(partition_table_start, 4 * 16) };

    let mut partition_table: [PartitionTableEntry; 4] = [PartitionTableEntry::default(); 4];

    for i in 0..4 {
        partition_table[i] = get_partition(partition_table_raw, i);
    }

    let fat_partition = partition_table.get(1).unwrap();
    // FAT32 with LBA
    assert!(fat_partition.partition_type == 0xc);

    let mut disk = disk::DiskAccess::new(
        disk_number,
        u64::from(fat_partition.logical_block_address),
        0,
    );

    let mut fs = fat::FileSystem::parse(disk);
    // todo: somehow not hardcode this ?
    let mut buffer = [0u8; 512 * 32];

    // LongNameEntry used even if name is just "stage3", entry afterwards is then
    // of type UNUSED
    for e in fs.read_root_dir(&mut buffer).filter_map(|e| e.ok()) {
        match e {
            fat::DirEntry::NormalDirEntry(e) => {
                println!("NormalEntry name: {}", e.filename)
            }
            fat::DirEntry::LongNameDirEntry(e) => {
                print!("\rLongNameEntry name: ");
                e.print_name();
                print!("\n");
                println!("Order: {} \n", e.order);
            }
        }
    }

    loop {
        hlt();
    }
}
