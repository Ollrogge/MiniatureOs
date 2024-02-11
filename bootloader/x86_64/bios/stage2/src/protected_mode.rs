use crate::GDT;
use common::BiosInfo;
use core::arch::asm;

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

pub fn enter_unreal_mode() {
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

pub fn enter_protected_mode_and_jump_to_stage3(entry_point: *const u8, info: &BiosInfo) {
    unsafe {
        // disable interrupts, set protection enabled bit in cr0
        asm!("cli", "mov eax, cr0", " or al, 1", " mov cr0, eax");
        asm!(
            // align the stack
            "and esp, 0xffffff00",
            // :e => specify that we want to use 32 bit registers
            // else the push would be 16 bit ig since we are still in real mode
            // push arguments
            "push {info:e}",
            // push entry point address
            "push {entry_point:e}",
            info = in(reg) info as *const _ as u32,
            entry_point = in(reg) entry_point as u32,
        );
        // Long jump to second entry in gdt (offset 8)
        // corresponds to the protected code segment descriptor we initialized
        // first argument is CS register, second is EIP
        asm!("ljmp $0x8, $2f", "2:", options(att_syntax));
        asm!(
            ".code32",

            // reload segment registers
            // 0x10 = offset 2 in gdt = data descriptor
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