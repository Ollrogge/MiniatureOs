//! Stage2 of the bootloader. Still in real mode
//! Tasks:
//! - Switch to unreal mode to be able to access more memory
//! - Load the next stages into memory by reading a FAT fs
//! - Query system memory & vesa information
//! - Switch to protected mode and jump to stage 3
#![no_std]
#![no_main]
use common::mbr::PARTITION_TABLE_ENTRY_COUNT;
use common::memory_map::{E820MemoryRegion, MemoryMap};
use common::{disk, fail, fat, hlt, mbr, memory_map, BiosFramebufferInfo, BiosInfo, Region};

use core::any::Any;
use core::borrow::Borrow;
use core::panic::PanicInfo;
use core::{arch::asm, slice};
use lazy_static::lazy_static;

mod print;
mod protected_mode;
mod vesa;
use protected_mode::*;

// 1 MiB
/// Basically the memory region we can use
const STAGE3_DST: *mut u8 = 0x0010_0000 as *mut u8;

use common::gdt::{GlobalDescriptorTable, SegmentDescriptor};

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(SegmentDescriptor::protected_mode_code_segment());
        gdt.add_entry(SegmentDescriptor::protected_mode_data_segment());
        gdt
    };
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        hlt();
    }
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(disk_number: u8, partition_table_start: *const u8) -> ! {
    start(disk_number, partition_table_start)
}

// Should look something like with QEMU:
//  Memory region, start: 0x0, length: 0x9fc00, type: Normal, attributes: 0x0
//  Memory region, start: 0x9fc00, length: 0x400, type: Reserved, attributes: 0x0
//  Memory region, start: 0xf0000, length: 0x10000, type: Reserved, attributes: 0x0
//  Memory region, start: 0x100000, length: 0x7ee0000, type: Normal, attributes: 0x0
//  Memory region, start: 0x7fe0000, length: 0x20000, type: Reserved, attributes: 0x0
//  Memory region, start: 0xfffc0000, length: 0x40000, type: Reserved, attributes: 0x0
//  Memory region, start: 0xfd00000000, length: 0x300000000, type: Reserved, attributes: 0x0
fn print_memory_map(map: &memory_map::MemoryMap) {
    for region in map.iter() {
        println!(
            "Memory region, start: {:#x}, length: {:#x}, type: {:?}, attributes: {:#x} ",
            region.start, region.length, region.typ, region.acpi_extended_attributes
        );
    }
}

fn start(disk_number: u8, partition_table_start: *const u8) -> ! {
    enter_unreal_mode();
    println!("Stage2 \r\n");

    let partition_table_raw = unsafe {
        slice::from_raw_parts(
            partition_table_start,
            mbr::PARTITION_TABLE_ENTRY_COUNT * mbr::PARTITION_TABLE_ENTRY_SIZE,
        )
    };
    let mut partition_table: [mbr::PartitionTableEntry; mbr::PARTITION_TABLE_ENTRY_COUNT] =
        [mbr::PartitionTableEntry::default(); mbr::PARTITION_TABLE_ENTRY_COUNT];

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

    let len = fs
        .try_load_file("stage3", STAGE3_DST)
        .expect("Failed to load stage3");

    println!("Stage3 loaded at: {:#p}, size: {:#x}", STAGE3_DST, len);

    let memory_map = memory_map::MemoryMap::get().expect("Failed to get memory map");
    print_memory_map(&memory_map);

    let vesa_info = vesa::VbeInfo::get().expect("Error getting Vesa info");
    let mode = vesa_info.get_best_mode(1280, 1024, 24);
    let mode_info = vesa::VbeModeInfo::get(mode).expect("Failed to get vesa mode info");

    vesa_info.set_mode(mode).expect("Failed to set vesa mode");

    // todo: kernel info
    let bios_info = BiosInfo::new(Region::new(0, 0), mode_info.to_framebuffer_info());

    enter_protected_mode_and_jump_to_stage3(STAGE3_DST, &bios_info);

    loop {
        hlt();
    }
}
