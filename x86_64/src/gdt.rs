//! Global Descriptor Table definitions
use crate::{interrupts, memory::VirtualAddress, tss::TaskStateSegment, PrivilegeLevel};
use bit_field::BitField;
use bitflags::bitflags;
use core::{arch::asm, convert::From, mem::size_of, ptr};

#[derive(Debug, Clone, Copy)]
pub struct SegmentSelector(u16);

impl SegmentSelector {
    pub fn new(idx: u16, rpl: PrivilegeLevel) -> Self {
        SegmentSelector(idx << 3 | rpl as u16)
    }

    pub fn raw(&self) -> u16 {
        self.0
    }
}

impl From<u16> for SegmentSelector {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemSegmentType {
    /// Local Descriptor Table (LDT)
    LDT = 0x2,
    /// 64-bit Task State Segment (TSS) available
    TssAvailable = 0x9,
    /// 64-bit Task State Segment (TSS) busy
    TssBusy = 0xB,
}

bitflags! {
    /// Combines the access byte and flags of a segment descriptor
    pub struct SegmentDescriptorFlags: u64 {
        /// Accessed bit. The CPU will set it when the segment is accessed
        /// unless set to 1 in advance.
        //
        /// Set by the processor if this segment has been accessed. Only cleared by software.
        /// _Setting_ this bit in software prevents GDT writes on first use.
        /// Best left set to 1 unless otherwise needed
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

/// There are two types of GDT entries in long mode: user segment descriptors and system segment descriptors.
///
/// User segment descriptors:
/// - In long mode, segmentation is largely unused for addressing. Thus, user segment descriptors do not contain an address.
/// - They span the entire 48-bit address space.
/// - Only flags such as present and descriptor privilege level (DPL) are relevant.
///
/// System segment descriptors:
/// - These are fully utilized in long mode.
/// - They contain a base address and a limit.
/// - To accommodate a 64-bit base address, system segment descriptors require a total of 128 bits.
pub enum SegmentDescriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}

impl SegmentDescriptor {
    pub fn new_user(flags: SegmentDescriptorFlags, limit: u32, base: u32) -> SegmentDescriptor {
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

        SegmentDescriptor::UserSegment(desc)
    }

    pub fn new_tss_segment(tss: &'static TaskStateSegment) -> SegmentDescriptor {
        let ptr = tss as *const _ as u64;
        let mut low = SegmentDescriptorFlags::PRESENT.bits();

        // base
        low.set_bits(16..=39, ptr.get_bits(0..24));
        low.set_bits(56..=63, ptr.get_bits(24..32));
        let mut high = 0x0;
        high.set_bits(0..=31, ptr.get_bits(32..64));

        // limit (contains size of TSS in bytes)
        low.set_bits(0..=15, (size_of::<TaskStateSegment>() - 1) as u64);

        // type
        low.set_bits(40..=43, SystemSegmentType::TssAvailable as u64);

        SegmentDescriptor::SystemSegment(low, high)
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
        SegmentDescriptor::new_user(flags, 0xFFFFF, 0)
    }

    pub fn protected_mode_data_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::PROTECTED_MODE
            | SegmentDescriptorFlags::GRANULARITY
            | SegmentDescriptorFlags::ACCESSED;

        SegmentDescriptor::new_user(flags, 0xFFFFF, 0)
    }

    pub fn long_mode_code_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::EXECUTABLE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::LONG_MODE
            | SegmentDescriptorFlags::ACCESSED
            | SegmentDescriptorFlags::GRANULARITY;

        // 64-bit mode, the Base and Limit values are ignored, each descriptor
        // covers the entire linear address space regardless of what they are set to.
        SegmentDescriptor::new_user(flags, 0, 0)
    }

    pub fn long_mode_data_segment() -> SegmentDescriptor {
        let flags = SegmentDescriptorFlags::READ_WRITE
            | SegmentDescriptorFlags::PRESENT
            | SegmentDescriptorFlags::USER_SEGMENT
            | SegmentDescriptorFlags::ACCESSED
            | SegmentDescriptorFlags::GRANULARITY;

        // 64-bit mode, the Base and Limit values are ignored, each descriptor
        // covers the entire linear address space regardless of what they are set to.
        SegmentDescriptor::new_user(flags, 0, 0)
    }

    pub fn kernel_code_segment() -> SegmentDescriptor {
        Self::long_mode_code_segment()
    }

    pub fn kernel_data_segment() -> SegmentDescriptor {
        Self::long_mode_data_segment()
    }

    pub fn descriptor_privilege_level(self) -> PrivilegeLevel {
        let value_low = match self {
            SegmentDescriptor::UserSegment(v) => v,
            SegmentDescriptor::SystemSegment(v, _) => v,
        };

        let dpl = (value_low >> 45) & 0b11;

        PrivilegeLevel::from(dpl as u8)
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

    pub fn add_entry(&mut self, entry: SegmentDescriptor) -> SegmentSelector {
        let idx = match entry {
            SegmentDescriptor::UserSegment(val) => self.push(val),
            SegmentDescriptor::SystemSegment(low, high) => {
                let idx = self.push(low);
                self.push(high);
                idx
            }
        };

        SegmentSelector::new(idx as u16, entry.descriptor_privilege_level())
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
            interrupts::disable();
            asm!("lgdt [{}]", in(reg) &desc, options(readonly, nostack, preserves_flags));
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
