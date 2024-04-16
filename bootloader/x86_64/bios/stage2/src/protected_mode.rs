use crate::GDT;
use common::BiosInfo;
use core::arch::asm;
use x86_64::{
    interrupts,
    register::{Cr0, Cr0Flags},
};

/// Enter unreal mode, a special operating mode which allows to access more
/// than 1 MiB of memory while running in real mode. This is achieved by
/// tweaking the descriptor caches.
///
/// The descriptor cache is a cache that keeps a copy of each segment descriptor
/// to save the processor from accessing the GDT for every memory access made.
///
/// For real mode the processor generates the entries internally as there is no
/// GDT in this mode. Interestingly, it does not update the segment limit field.
///
/// => One can use this behavior to address more memory in real mode by entering
/// protected mode, changing the limit and then changing back to real mode.
///
pub fn enter_unreal_mode() {
    let ds: u16;
    let ss: u16;

    // save old values
    unsafe {
        asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
    }

    // load GDT with protected mode descriptors
    GDT.clear_interrupts_and_load();

    // set protected mode bit
    let cr0 = Cr0::read_raw();
    unsafe {
        Cr0::update(|val| *val |= Cr0Flags::PROTECTED_MODE_ENABLE);
    }

    // set segment registers, which are indexes into the GDT.
    // This will fetch and cache the corresponding descriptor from the GDT.
    // In real mode, even though the CPU calculates physical addresses the
    // real-mode way, any access to memory still respects the limits and
    // characteristics of the cached descriptors.

    // With the adjusted limit one can simply use 32 bit offsets for the
    // segment:offset memory addressing in real mode and by that access
    // 4 GiB of memory.

    // mov descriptor (0x10 / 2) = data segment descriptor to ds and ss
    unsafe {
        asm!("mov {0}, 0x10", "mov ds, {0}", "mov ss, {0}", out(reg) _);
    }

    // unset protected mode bit again
    unsafe {
        Cr0::write_raw(cr0);
    }

    // recover old segment registers, turn on interrupts
    unsafe {
        asm!("mov ds, {0:x}", in(reg) ds, options(nostack, preserves_flags));
        asm!("mov ss, {0:x}", in(reg) ss, options(nostack, preserves_flags));
        asm!("sti");
    }
}

pub fn enter_protected_mode_and_jump_to_stage3(entry_point: *const u8, info: &BiosInfo) {
    unsafe {
        interrupts::disable();
        Cr0::update(|val| *val |= Cr0Flags::PROTECTED_MODE_ENABLE);
    }

    unsafe {
        asm!(
            // 16 bit align the stack
            "and esp, 0xffffff00",
            // :e => specify that we want to use 32 bit registers
            // else the push would be 16 bit since we are still in real mode
            // push arguments
            "push {info:e}",
            // push entry point address
            "push {entry_point:e}",
            info = in(reg) info as *const _ as u32,
            entry_point = in(reg) entry_point as u32,
        );

        // Long jump. Long jumps can jump to an address in a different code segment.
        // First argument specifies the segment selector which points points to an
        // entry in the Global Descriptor Table (GDT) that defines the properties
        // of the segment to which control is being transferred
        // Second argument is the jump target. Label "2" in this case
        // changes the value in CS register
        asm!("ljmp $0x8, $2f", options(att_syntax));
        asm!(
            "2:",
            ".code32",

            // reload segment registers
            // 0x10 = offset 2 in gdt = data descriptor
            "mov {0}, 0x10",
            "mov ds, {0}",
            "mov es, {0}",
            "mov ss, {0}",

            // jump to third stage, by popping entry point from stack
            // (which was pushed above)
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
