#![no_std]
#![no_main]
use common::{disk, fail, fat, hlt, mbr, memory_map, println};

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

// Should look something like:
// Memory region, start: 0x0, length: 0x9fc00, type: Normal, attributes: 0x0
// Memory region, start: 0x9fc00, length: 0x400, type: Reserved, attributes: 0x0
// Memory region, start: 0xf0000, length: 0x10000, type: Reserved, attributes: 0x0
// Memory region, start: 0x100000, length: 0x7ee0000, type: Normal, attributes: 0x0
// Memory region, start: 0x7fe0000, length: 0x20000, type: Reserved, attributes: 0x0
// Memory region, start: 0xfffc0000, length: 0x40000, type: Reserved, attributes: 0x0
// Memory region, start: 0xfd00000000, length: 0x300000000, type: Reserved, attributes: 0x0
fn print_memory_map(map: &[memory_map::E820MemoryRegion]) {
    for region in map {
        println!(
            "Memory region, start: {:#x}, length: {:#x}, type: {:?}, attributes: {:#x} ",
            region.start, region.length, region.typ, region.acpi_extended_attributes
        );
    }
}

fn start(disk_number: u8, partition_table_start: *const u8) -> ! {
    enter_unreal_mode();
    println!("Stage2 \r\n");

    let partition_table_raw = unsafe { slice::from_raw_parts(partition_table_start, 4 * 16) };
    let mut partition_table: [mbr::PartitionTableEntry; 4] =
        [mbr::PartitionTableEntry::default(); 4];

    for i in 0..4 {
        partition_table[i] = mbr::get_partition(partition_table_raw, i);
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

    let stage3 = match fs.find_file_in_root_dir("stage3") {
        Some(v) => v,
        _ => panic!("Failed to find stage3"),
    };

    fs.try_load_file("stage3", STAGE3_DST)
        .expect("Failed to load stage3");

    println!("Stage3 loaded at: {:#p}", STAGE3_DST);

    let memory_map = memory_map::load_memory_map().expect("Failed to load memory map");

    print_memory_map(memory_map);

    loop {
        hlt();
    }
}
