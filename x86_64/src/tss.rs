//! This module implements functionality for the x86_64 task state segment (TSS)
//!
//! - In long mode the TSS not store information on a task's execution state,
//! instead it is used to store the Interrupt Stack Table.
//! - The interrupt stack table contains 7 pointers to known "good" stacks
//! - This is needed to e.g. avoid the cpu pushing the exception frame onto
//! the stack when access to the stack causes page faults, which would then cause
//! a triple fault
//! - Also contains 3 further stacks used to load the stack when a privilege
//! level change occurs
//!
//!
//!
//!
use crate::{gdt::SegmentSelector, memory::VirtualAddress};
use core::{arch::asm, mem::size_of};

pub const DOUBLE_FAULT_IST_IDX: usize = 0x0;

/// TaskStateSegment struct
#[repr(C, packed(4))]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub privilege_stack_table: [VirtualAddress; 3],
    reserved_2: u64,
    /// Interrupt stack table (IST)
    pub interrupt_stack_table: [VirtualAddress; 7],
    reserved_3: u64,
    reserved_4: u16,
    /// I/O map base field. Contains a 16-bit offset from the base of the
    /// TSS to the I/O Permission Bit Map
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub fn new() -> Self {
        TaskStateSegment {
            privilege_stack_table: [VirtualAddress::new(0); 3],
            interrupt_stack_table: [VirtualAddress::new(0); 7],
            iomap_base: size_of::<TaskStateSegment>() as u16,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
            reserved_4: 0,
        }
    }

    /// Load the Task Register (TR) with a segment selector that points to a
    /// Task State Segment (TSS) descriptor in the Global Descriptor Table (GDT).
    /// Note that loading a TSS segment selector marks the corresponding TSS
    /// Descriptor in the GDT as "busy", preventing it from being loaded again
    /// (either on this CPU or another CPU). TSS structures (including Descriptors
    /// and Selectors) should generally be per-CPU.
    ///
    /// Calling this function with a busy TSS selector results in a general protection exception.
    ///
    /// ## Safety
    ///
    /// This function is unsafe because the caller must ensure that the given
    /// `SegmentSelector` points to a valid TSS entry in the GDT and that the
    /// corresponding data in the TSS is valid.
    pub unsafe fn load(sel: SegmentSelector) {
        unsafe {
            asm!("ltr {0:x}", in(reg) sel.raw(), options(nostack, preserves_flags));
        }
    }
}
