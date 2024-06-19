//! This module implements functionality for the x86 interrupt descriptor table
//!
//! There are two kinds of interrupts: interrupts due to bad (exceptions) code
//! or the ones that occur to handle events unrelated to currently executing code
//!
//!     Faults: These can be corrected and the program may continue as if nothing happened
//!     Traps: Traps are reported immediately after the execution of the trapping instruction.
//!     Aborts: Some severe unrecoverable error.
//!
//! Interrupt vs trap gate: when you call an interrupt-gate, interrupts get disabled,
//! and when you call a trap-gate, they don't
//!
use crate::{const_assert, gdt::SegmentSelector, println, register::CS, PrivilegeLevel};
use bit_field::BitField;
use core::{arch::asm, default::Default, mem::size_of};

#[derive(Debug, Clone, Copy)]
pub struct InterruptDescriptorOptions(u16);

impl Default for InterruptDescriptorOptions {
    fn default() -> Self {
        // dpl = 0, type = 0xb1110
        const DEFAULT_INTERRUPT_GATE_64: u16 = 0xe << 8;
        InterruptDescriptorOptions(DEFAULT_INTERRUPT_GATE_64)
    }
}

impl InterruptDescriptorOptions {
    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    pub fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.0.set_bit(8, !disable);
        self
    }

    pub fn set_privilege_level(&mut self, level: PrivilegeLevel) -> &mut Self {
        self.0.set_bits(13..=14, level as u16);
        self
    }

    /// Sets the interrupt stack table index.
    ///
    /// This is an offset into the Interrupt Stack Table, which is stored in the Task State Segment
    pub fn set_interrupt_stack_index(&mut self, index: u16) -> &mut Self {
        // The hardware IST index starts at 1, but our software IST index
        // starts at 0. Therefore we need to add 1 here.
        self.0.set_bits(0..=2, index + 1);
        self
    }
}

pub type HandlerFunc = extern "C" fn() -> !;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InterruptDescriptor {
    pointer_low: u16,
    segment_selector: SegmentSelector,
    options: InterruptDescriptorOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl InterruptDescriptor {
    pub fn missing() -> Self {
        Self {
            pointer_low: 0,
            segment_selector: SegmentSelector::new(0, PrivilegeLevel::Ring0),
            options: InterruptDescriptorOptions::default(),
            pointer_middle: 0,
            pointer_high: 0,
            reserved: 0,
        }
    }

    /// Sets hanlder address for IDT entry
    ///
    /// # Safety
    ///
    /// Unsafe because caller must ensure that the HandlerFunc passed is valid
    pub unsafe fn set_handler_function(
        &mut self,
        handler: HandlerFunc,
    ) -> &mut InterruptDescriptorOptions {
        let handler_address = handler as u64;
        self.pointer_low = handler_address as u16;
        self.pointer_middle = (handler_address >> 16) as u16;
        self.pointer_high = (handler_address >> 32) as u32;

        self.segment_selector = CS::read().into();

        self.options.set_present(true);

        &mut self.options
    }
}

/// IDT descriptor which will be loaded into the IDT register
#[repr(C, packed)]
pub struct InterruptTableDescriptor {
    /// The size of the idt - 1 in bytes
    size: u16,
    /// Address of the idt
    base: u64,
}

#[derive(Clone, Debug)]
#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    pub divide_error: InterruptDescriptor,
    pub debug: InterruptDescriptor,
    pub non_maskable_interrupt: InterruptDescriptor,
    pub breakpoint: InterruptDescriptor,
    pub overflow: InterruptDescriptor,
    pub bound_range_exceeded: InterruptDescriptor,
    pub invalid_opcode: InterruptDescriptor,
    pub device_not_available: InterruptDescriptor,
    pub double_fault: InterruptDescriptor,
    coprocessor_segment_overrun: InterruptDescriptor,
    pub invalid_tss: InterruptDescriptor,
    pub segment_not_present: InterruptDescriptor,
    pub stack_segment_fault: InterruptDescriptor,
    pub general_protection_fault: InterruptDescriptor,
    pub page_fault: InterruptDescriptor,
    reserved_1: InterruptDescriptor,
    pub x87_floating_point: InterruptDescriptor,
    pub alignment_check: InterruptDescriptor,
    pub machine_check: InterruptDescriptor,
    pub simd_floating_point: InterruptDescriptor,
    pub virtualization: InterruptDescriptor,
    pub control_protection_exception: InterruptDescriptor,
    reserved_2: [InterruptDescriptor; 6],
    pub hypervisor_injection_exception: InterruptDescriptor,
    pub vmm_communication_exception: InterruptDescriptor,
    pub security_exception: InterruptDescriptor,
    reserved_3: InterruptDescriptor,
    pub interrupts: [InterruptDescriptor; 256 - 32],
}
const_assert!(
    size_of::<InterruptDescriptorTable>() == 256 * size_of::<InterruptDescriptor>(),
    "IDT has invalid size"
);

impl InterruptDescriptorTable {
    // Static lifetime to make sure idt will live long enough and not e.g.
    // be initialized on the stack stack inside a function which causes
    // undefined behavior when the function returns
    pub fn load(&'static self) {
        let desc = InterruptTableDescriptor {
            size: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };

        let val = desc.base;
        println!("Idt addr: {:x}", val);

        unsafe {
            lidt(&desc);
        };
    }
}

impl Default for InterruptDescriptorTable {
    fn default() -> Self {
        Self {
            divide_error: InterruptDescriptor::missing(),
            debug: InterruptDescriptor::missing(),
            non_maskable_interrupt: InterruptDescriptor::missing(),
            breakpoint: InterruptDescriptor::missing(),
            overflow: InterruptDescriptor::missing(),
            bound_range_exceeded: InterruptDescriptor::missing(),
            invalid_opcode: InterruptDescriptor::missing(),
            device_not_available: InterruptDescriptor::missing(),
            double_fault: InterruptDescriptor::missing(),
            coprocessor_segment_overrun: InterruptDescriptor::missing(),
            invalid_tss: InterruptDescriptor::missing(),
            segment_not_present: InterruptDescriptor::missing(),
            stack_segment_fault: InterruptDescriptor::missing(),
            general_protection_fault: InterruptDescriptor::missing(),
            page_fault: InterruptDescriptor::missing(),
            reserved_1: InterruptDescriptor::missing(),
            x87_floating_point: InterruptDescriptor::missing(),
            alignment_check: InterruptDescriptor::missing(),
            machine_check: InterruptDescriptor::missing(),
            simd_floating_point: InterruptDescriptor::missing(),
            virtualization: InterruptDescriptor::missing(),
            control_protection_exception: InterruptDescriptor::missing(),
            reserved_2: [InterruptDescriptor::missing(); 6],
            hypervisor_injection_exception: InterruptDescriptor::missing(),
            vmm_communication_exception: InterruptDescriptor::missing(),
            security_exception: InterruptDescriptor::missing(),
            reserved_3: InterruptDescriptor::missing(),
            interrupts: [InterruptDescriptor::missing(); 256 - 32],
        }
    }
}

/// Loads the descriptor into the interrupt descriptor table register
///
/// # Safety
///
/// Unsafe because incorrect usage can result in undefined behavior
unsafe fn lidt(descriptor: &InterruptTableDescriptor) {
    asm!("lidt [{}]", in(reg) descriptor, options(readonly, nostack, preserves_flags));
}
