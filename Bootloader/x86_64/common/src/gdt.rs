//! Global Descriptor Table definitions
use bit_field::BitField;
use bitflags::bitflags;
use core::arch::asm;

bitflags! {
    /// Combines the access byte and flags of a segment descriptor
    struct SegmentDescriptorFlags: u64 {
        /// Readable if code segment, read and writable if data segment
        const READ_WRITE = 1 << 41;
        const CONFORMING = 1 << 42;
        const EXECUTABLE = 1 << 43;
        /// Descriptor type. clear = system segment, set = code or data
        const CODE_OR_DATA = 1 << 44;
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
            | SegmentDescriptorFlags::CODE_OR_DATA
            | SegmentDescriptorFlags::PROTECTED_MODE
            | SegmentDescriptorFlags::GRANULARITY;

        SegmentDescriptor::new(flags, 0xFFFFF, 0)
    }

    pub fn protected_mode_data_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::CODE_OR_DATA
            | SegmentDescriptorFlags::PROTECTED_MODE
            | SegmentDescriptorFlags::GRANULARITY;

        SegmentDescriptor::new(flags, 0xFFFFF, 0)
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

    pub fn clear_interrupts_and_load(&'static self) {
        let desc = GlobalDescriptorTableDescriptor::new(self);

        unsafe {
            asm!("cli", "lgdt [{}]", in(reg) &desc, options(readonly, nostack, preserves_flags));
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
