#![no_std]
#![feature(hint_must_use)]
#![feature(naked_functions)]
pub mod gdt;
pub mod idt;
pub mod instructions;
pub mod interrupts;
pub mod memory;
pub mod mutex;
pub mod paging;
pub mod port;
pub mod print;
pub mod register;
pub mod tss;
pub mod uart;

use core::convert::From;

#[repr(u8)]
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

impl From<u8> for PrivilegeLevel {
    fn from(v: u8) -> Self {
        match v {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("Invalid privilege level"),
        }
    }
}
