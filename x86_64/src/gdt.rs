//! Global Descriptor Table definitions
use bit_field::BitField;
use bitflags::bitflags;
use core::arch::asm;
use core::ptr;

use crate::memory::VirtualAddress;

bitflags! {
    /// Combines the access byte and flags of a segment descriptor
    pub struct SegmentDescriptorFlags: u64 {
        /// Accessed bit. The CPU will set it when the segment is accessed
        /// unless set to 1 in advance.
        //
        // This means that in case the GDT descriptor is stored in read only pages
        // and this bit is set to 0, the CPU trying to set this bit will trigger
        // a page fault. Best left set to 1 unless otherwise needed.
        const ACCESSED = 1 << 40;
        /// Readable if code segment, read and writable if data segment
        const READ_WRITE = 1 << 41;
        const CONFORMING = 1 << 42;
        const EXECUTABLE = 1 << 43;
        /// Descriptor type. clear = system segment, set = code or data
        const USER_SEGMENT = 1 << 44;
        /// Entry refers to valid segment
        const PRESENT = 1 << 47;
        /// Set if descriptor defines a 64-bit code segment
        const LONG_MODE = 1 << 53;
        /// Set if descriptor defines a 32-bit protected mode segment
        const PROTECTED_MODE = 1 << 54;
        /// If set limit is in 4 KiB blocks, else byte blocks
        const GRANULARITY = 1 << 55;
   }
}

pub struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    pub fn new(flags: SegmentDescriptorFlags, limit: u32, base: u32) -> SegmentDescriptor {
        let limit_low = limit & 0xFFFF;
        let limit_high = (limit >> 16) & 0b1111;
        let base_low = base & 0xFFFFFF;
        let base_high = (base >> 24) & 0xFF;

        let mut desc = flags.bits();

        if base != 0 {
            desc.set_bits(16..=39, base_low.into());
            desc.set_bits(56..=63, base_high.into());
        }

        desc.set_bits(0..=15, limit_low.into());
        desc.set_bits(48..=51, limit_high.into());

        SegmentDescriptor(desc)
    }

    pub fn protected_mode_code_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::EXECUTABLE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::PROTECTED_MODE
            | SegmentDescriptorFlags::GRANULARITY
            | SegmentDescriptorFlags::ACCESSED;

        // Base = A 32-bit value containing the linear address where the segment begins.
        //
        // Limit = A 20-bit value, tells the maximum addressable unit, either
        // in 1 byte units, or in 4KiB pages.
        // Hence, if you choose page granularity and set the Limit value to 0xFFFFF
        // the segment will span the full 4 GiB address space in 32-bit mode.
        SegmentDescriptor::new(flags, 0xFFFFF, 0)
    }

    pub fn protected_mode_data_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::PROTECTED_MODE
            | SegmentDescriptorFlags::GRANULARITY
            | SegmentDescriptorFlags::ACCESSED;

        SegmentDescriptor::new(flags, 0xFFFFF, 0)
    }

    pub fn long_mode_code_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::EXECUTABLE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::LONG_MODE
            | SegmentDescriptorFlags::ACCESSED;

        // 64-bit mode, the Base and Limit values are ignored, each descriptor
        // covers the entire linear address space regardless of what they are set to.
        SegmentDescriptor::new(flags, 0, 0)
    }

    pub fn long_mode_data_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::ACCESSED;

        SegmentDescriptor::new(flags, 0, 0)
    }

    pub fn kernel_code_segment() -> SegmentDescriptor {
        Self::long_mode_code_segment()
    }

    pub fn kernel_data_segment() -> SegmentDescriptor {
        Self::long_mode_data_segment()
    }
}

const GLOBAL_DESCRIPTOR_TABLE_ENTRY_COUNT: usize = 0x8;
#[derive(Debug)]
#[repr(C)]
pub struct GlobalDescriptorTable {
    entries: [u64; GLOBAL_DESCRIPTOR_TABLE_ENTRY_COUNT],
    size: usize,
}

impl GlobalDescriptorTable {
    pub fn new() -> GlobalDescriptorTable {
        GlobalDescriptorTable {
            entries: [0x0; GLOBAL_DESCRIPTOR_TABLE_ENTRY_COUNT],
            // entry 0 is null by default
            size: 1,
        }
    }

    pub fn initialize_at_address(address: VirtualAddress) -> &'static mut GlobalDescriptorTable {
        let gdt_ptr: *mut GlobalDescriptorTable = address.as_mut_ptr();

        unsafe {
            ptr::write(gdt_ptr, Self::new());
        }

        unsafe { &mut *gdt_ptr }
    }

    pub fn add_entry(&mut self, entry: SegmentDescriptor) {
        self.push(entry.0);
    }

    fn push(&mut self, value: u64) -> usize {
        if self.size < self.entries.len() {
            let idx = self.size;
            self.entries[idx] = value;
            self.size += 1;
            idx
        } else {
            panic!("GDT full");
        }
    }

    pub fn clear_interrupts_and_load(&self) {
        let desc = GlobalDescriptorTableDescriptor::new(self);

        unsafe {
            asm!("cli", "lgdt [{}]", in(reg) &desc, options(readonly, nostack, preserves_flags));
        }
    }

    pub fn load(&self) {
        let desc = GlobalDescriptorTableDescriptor::new(self);
        unsafe {
            asm!("lgdt [{}]", in(reg) &desc, options(readonly, nostack, preserves_flags));
        }
    }
}

#[repr(C, packed(2))]
pub struct GlobalDescriptorTableDescriptor {
    size: u16,
    base: *const GlobalDescriptorTable,
}

impl GlobalDescriptorTableDescriptor {
    pub fn new(table: &GlobalDescriptorTable) -> GlobalDescriptorTableDescriptor {
        GlobalDescriptorTableDescriptor {
            size: (table.size * 8 - 1) as u16,
            base: table,
        }
    }
}
