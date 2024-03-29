#![no_std]
pub mod frame_allocator;
pub mod gdt;
pub mod idt;
pub mod instructions;
pub mod interrupts;
pub mod memory;
pub mod mutex;
pub mod paging;
pub mod print;
pub mod register;
pub mod uart;

/// CPU privilege levels, or also "rings"
pub enum PrivilegeLevel {
    /// Supervisor mode. Least protection, most access to resources
    Ring0,
    /// Mostly used by device drivers (or not at all?)
    Ring1,
    /// Mostly used by device drivers (or not at all?)
    Ring2,
    /// Userland mode. Most protections, least access to resources
    Ring3,
}
