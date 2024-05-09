//! This is the stage3 code of the bootloader. This codes executes in protected mode.
//!
//! Some notes on protected mode:
//!     - Cant use BIOS functions anymore. Therefore need a UART driver for text output from this point on
//!     - Segment registers are now interpreted as indexes into the GDT (bits 3-15)
#![no_std]
#![no_main]
use common::{hlt, BiosInfo};
use core::{arch::asm, panic::PanicInfo};
use lazy_static::lazy_static;
use x86_64::{
    gdt::{GlobalDescriptorTable, SegmentDescriptor},
    println,
};

mod paging;

// This is going to be placed in the binary image which is loaded into RAM
lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(SegmentDescriptor::long_mode_code_segment());
        gdt.add_entry(SegmentDescriptor::long_mode_data_segment());
        gdt
    };
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {
        hlt();
    }
}

fn jump_to_stage4(info: &BiosInfo) {
    unsafe {
        asm!(
            // align the stack
            "and esp, 0xffffff00",
            // :e => specify that we want to use 32 bit registers
            // else the push would be 16 bit ig since we are still in real mode
            // push arguments (extended to 64 bit), therefore we push 0 twice
            // little-endian, stack grows downwards to push bits at higher address first
            "push 0",
            "push {info:e}",
            // push entry point address
            "push 0",
            "push {entry_point:e}",
            info = in(reg) info as *const _ as u32,
            entry_point = in(reg) info.stage4.start as u32,
        );
        // Long jump. Long jumps can jump to an address in a different code segment.
        // First argument specifies the segment selector which points points to an
        // entry in the Global Descriptor Table (GDT) that defines the properties
        // of the segment to which control is being transferred
        // Second argument is the jump target. Label "2" in this case
        // changes the value in CS register
        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));
        asm!(
            ".code64",

            // reload segment registers
            // 0x10 = offset 2 in gdt = data descriptor
            "mov {0}, 0x10",
            "mov ds, {0}",
            "mov es, {0}",
            "mov ss, {0}",

            // jump to fourth
            "pop rax",
            "pop rdi",
            "call rax",

            // enter endless loop in case third stage returns
            "2:",
            "jmp 2b",
            out(reg) _,
            out("rax")_,
            out("rdi")_
        );
    }
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &BiosInfo) -> ! {
    start(info);
}

fn start(info: &BiosInfo) -> ! {
    println!("Stage3");
    // this also switches to long mode
    paging::init();

    // now we are in long mode but in 32-bit compatibility submode, to enter
    // 64-bit long mode load GDT with 64 bit segment descriptors for code and data
    GDT.clear_interrupts_and_load();

    jump_to_stage4(info);

    loop {
        hlt();
    }
}
