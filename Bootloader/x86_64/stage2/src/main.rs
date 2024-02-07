#![no_std]
#![no_main]
use common::memory_map::{E820MemoryRegion, MemoryMap};
use common::{
    disk, fail, fat, hlt, mbr, memory_map, println, vesa, BiosFramebufferInfo, BiosInfo, Region,
};

use core::any::Any;
use core::borrow::Borrow;
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

fn set_protected_mode_bit() -> u32 {
    let mut cr0: u32;
    unsafe {
        asm!("mov {:e}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    }
    let cr0_protected = cr0 | 1;
    write_cr0(cr0_protected);
    cr0
}

fn write_cr0(val: u32) {
    unsafe { asm!("mov cr0, {:e}", in(reg) val, options(nostack, preserves_flags)) };
}

fn enter_unreal_mode() {
    let ds: u16;
    let ss: u16;

    unsafe {
        asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
    }

    GDT.clear_interrupts_and_load();

    // set protected mode bit
    let cr0 = set_protected_mode_bit();

    // load GDT
    // mov descriptor (0x10 / 2) = data segment descriptor to ds and ss
    unsafe {
        asm!("mov {0}, 0x10", "mov ds, {0}", "mov ss, {0}", out(reg) _);
    }

    // unset protected mode bit again
    write_cr0(cr0);

    unsafe {
        asm!("mov ds, {0:x}", in(reg) ds, options(nostack, preserves_flags));
        asm!("mov ss, {0:x}", in(reg) ss, options(nostack, preserves_flags));
        asm!("sti");
    }
}

fn enter_protected_mode_and_jump_to_stage3(entry_point: *const u8, info: &BiosInfo) {
    unsafe {
        // disable interrupts, set protection enabled bit in cr0
        asm!("cli", "mov eax, cr0", " or al, 1", " mov cr0, eax");
        asm!(
            // align the stack
            "and esp, 0xffffff00",
            // push arguments
            "push {info:e}",
            // push entry point address
            "push {entry_point:e}",
            info = in(reg) info as *const _ as u32,
            entry_point = in(reg) entry_point as u32,
        );
        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));
        asm!(
            ".code32",

            // reload segment registers
            "mov {0}, 0x10",
            "mov ds, {0}",
            "mov es, {0}",
            "mov ss, {0}",

            // jump to third stage
            "pop {1}",
            "call {1}",

            // enter endless loop in case third stage returns
            "2:",
            "jmp 2b",
            out(reg) _,
            out(reg) _,
        );
    }
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(disk_number: u8, partition_table_start: *const u8) -> ! {
    start(disk_number, partition_table_start)
}

// Should look something like:
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

    fs.try_load_file("stage3", STAGE3_DST)
        .expect("Failed to load stage3");

    println!("Stage3 loaded at: {:#p}", STAGE3_DST);

    let memory_map = memory_map::MemoryMap::get().expect("Failed to get memory map");
    print_memory_map(&memory_map);

    let vesa_info = vesa::VesaInfo::get().expect("Error getting Vesa info");
    let mode = vesa_info.get_best_mode(1280, 1024, 24);
    let mode_info = vesa::VesaModeInfo::get(mode).expect("Failed to get vesa mode info");

    //vesa_info.set_mode(mode).expect("Failed to set vesa mode");

    // todo: kernel info
    let bios_info = BiosInfo::new(Region::new(0, 0), mode_info.to_framebuffer_info());

    println!("Waiting");
    loop {
        hlt();
    }

    enter_protected_mode_and_jump_to_stage3(STAGE3_DST, &bios_info);

    loop {
        hlt();
    }
}
