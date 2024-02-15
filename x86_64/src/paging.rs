use bitflags::bitflags;
use core::slice;

use crate::memory::{Address, VirtualAddress};

bitflags! {
    /// Possible flags for a page table entry.
    pub struct PageTableFlags: u64 {
        /// Specifies whether the mapped frame or page table is loaded in memory.
        const PRESENT =         1;
        /// Controls whether writes to the mapped frames are allowed.
        ///
        /// If this bit is unset in a level 1 page table entry, the mapped frame is read-only.
        /// If this bit is unset in a higher level page table entry the complete range of mapped
        /// pages is read-only.
        const WRITABLE =        1 << 1;
        /// Controls whether accesses from userspace (i.e. ring 3) are permitted.
        const USER_ACCESSIBLE = 1 << 2;
        /// If this bit is set, a “write-through” policy is used for the cache, else a “write-back”
        /// policy is used.
        const WRITE_THROUGH =   1 << 3;
        /// Disables caching for the pointed entry is cacheable.
        const NO_CACHE =        1 << 4;
        /// Set by the CPU when the mapped frame or page table is accessed.
        const ACCESSED =        1 << 5;
        /// Set by the CPU on a write to the mapped frame.
        const DIRTY =           1 << 6;
        /// Specifies that the entry maps a huge frame instead of a page table. Only allowed in
        /// P2 or P3 tables.
        const HUGE_PAGE =       1 << 7;
        /// Indicates that the mapping is present in all address spaces, so it isn't flushed from
        /// the TLB on an address space switch.
        const GLOBAL =          1 << 8;
        /// Forbid code execution from the mapped frames.
        ///
        /// Can be only used when the no-execute page protection feature is enabled in the EFER
        /// register.
        const NO_EXECUTE =      1 << 63;
    }
}

const TABLE_ENTRY_COUNT: usize = 512;
const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn new(val: u64) -> PageTableEntry {
        PageTableEntry(val)
    }
}

#[repr(align(4096))]
#[derive(Clone, Copy)]
pub struct PageTable {
    pub entries: [PageTableEntry; TABLE_ENTRY_COUNT],
}

impl PageTable {
    pub const SIZE: usize = core::mem::size_of::<Self>();
    pub const fn empty() -> Self {
        Self {
            entries: [PageTableEntry(0); TABLE_ENTRY_COUNT],
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter_mut(&mut self) -> slice::IterMut<'_, PageTableEntry> {
        self.entries.iter_mut()
    }
}

// A Pagetable that assumes that physical memory is mapped at a constant offset
// in virtual address space
pub struct OffsetPageTable {
    inner: PageTable,
    offset: VirtualAddress,
}

impl OffsetPageTable {
    pub fn new(inner: PageTable, offset: VirtualAddress) -> Self {
        Self { inner, offset }
    }

    pub fn physical_frame_offset(&self) -> u64 {
        self.offset.as_u64()
    }
}

/// A Pagetable that requires that physical memory is are mapped to some virtual address
///
/// In this case the tables don't need to be continuous in memory at a specific offset
/// from their physical address
pub struct MappedPageTable {}
