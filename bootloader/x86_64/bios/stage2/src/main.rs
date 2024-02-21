//! Stage2 of the bootloader. Still in real mode
//! Tasks:
//! - Switch to unreal mode to be able to access more memory
//! - Load the next stages into memory by reading a FAT fs
//! - Query system memory & vesa information
//! - Switch to protected mode and jump to stage 3
#![no_std]
#![no_main]
use common::{fail, hlt, mbr, BiosFramebufferInfo, BiosInfo, E820MemoryRegion};
use core::{panic::PanicInfo, slice};
use lazy_static::lazy_static;
use x86_64::{
    gdt::{GlobalDescriptorTable, SegmentDescriptor},
    memory::Region,
};

mod dap;
mod disk;
mod fat;
mod memory_map;
mod print;
mod protected_mode;
mod vesa;
use crate::memory_map::MEMORY_MAP;
use memory_map::MemoryMap;
use protected_mode::*;

const STAGE3_DST: *mut u8 = 0x0010_0000 as *mut u8;
const STAGE4_DST: *mut u8 = 0x0012_0000 as *mut u8;
const KERNEL_DST: *mut u8 = 0x0020_0000 as *mut u8;

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
fn print_memory_map(map: &MemoryMap) {
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

    let disk = disk::DiskAccess::new(
        disk_number,
        u64::from(fat_partition.logical_block_address),
        0,
    );

    let mut fs = fat::FileSystem::parse(disk);

    let stage3_len = fs
        .try_load_file("stage3", STAGE3_DST)
        .expect("Failed to load stage3");

    println!(
        "Stage3 loaded at: {:#p}, size: {:#x}",
        STAGE3_DST, stage3_len
    );

    let stage4_len = fs
        .try_load_file("stage4", STAGE4_DST)
        .expect("Failed to load stage4");

    println!(
        "Stage4 loaded at: {:#p}, size: {:#x}",
        STAGE4_DST, stage4_len
    );

    let kernel_len = fs
        .try_load_file("kernel", KERNEL_DST)
        .expect("Failed to load kernel");

    println!(
        "Kernel loaded at: {:#p}, size: {:#x}",
        KERNEL_DST, kernel_len
    );

    let memory_map = MemoryMap::get().expect("Failed to get memory map");
    print_memory_map(&memory_map);

    let vesa_info = vesa::VbeInfo::get().expect("Error getting Vesa info");
    let mode = vesa_info
        .get_best_mode(1280, 1024, 24)
        .expect("Unable to get vesa mode");
    let mode_info = vesa::VbeModeInfo::get(mode).expect("Failed to get vesa mode info");

    vesa_info.set_mode(mode).expect("Failed to set vesa mode");

    // todo: kernel info
    let bios_info = BiosInfo {
        stage4: Region::new(STAGE4_DST as u64, stage4_len as u64),
        kernel: Region::new(KERNEL_DST as u64, kernel_len as u64),
        framebuffer: mode_info.to_framebuffer_info(),
        last_physical_address: KERNEL_DST as u64 + kernel_len as u64,
        memory_map_address: memory_map.map[0..memory_map.size].as_ptr() as u64,
        memory_map_size: memory_map.size as u64,
    };

    enter_protected_mode_and_jump_to_stage3(STAGE3_DST, &bios_info);

    loop {
        hlt();
    }
}
