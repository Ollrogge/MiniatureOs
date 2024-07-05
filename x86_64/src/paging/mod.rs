use crate::{
    instructions,
    memory::{
        Address, FrameAllocator, Page, PageSize, PhysicalAddress, PhysicalFrame, Size2MiB,
        Size4KiB, VirtualAddress,
    },
};
use bit_field::BitField;
use bitflags::bitflags;
use core::{
    ops::{Index, IndexMut},
    ptr,
    result::Result,
    slice,
};

pub mod bump_frame_allocator;
pub mod linked_list_frame_allocator;
pub mod mapped_page_table;
pub mod offset_page_table;

bitflags! {
    /// Possible flags for a page table entry.
    #[derive(Clone, Copy)]
    pub struct PageTableEntryFlags: u64 {
        const NONE = 0;
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
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_9 =           1 << 9;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_10 =          1 << 10;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_11 =          1 << 11;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_52 =          1 << 52;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_53 =          1 << 53;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_54 =          1 << 54;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_55 =          1 << 55;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_56 =          1 << 56;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_57 =          1 << 57;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_58 =          1 << 58;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_59 =          1 << 59;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_60 =          1 << 60;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_61 =          1 << 61;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_62 =          1 << 62;
        /// Forbid code execution from the mapped frames.
        ///
        /// Can be only used when the no-execute page protection feature is enabled in the EFER
        /// register.
        const NO_EXECUTE =      1 << 63;
    }
}

const TABLE_ENTRY_COUNT: usize = 512;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn new(val: u64) -> PageTableEntry {
        PageTableEntry(val)
    }

    pub fn is_present(&self) -> bool {
        (self.0 & PageTableEntryFlags::PRESENT.bits()) != 0
    }

    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0.get_bits(12..48) << 12)
    }

    pub fn physical_frame(&self) -> PhysicalFrame {
        PhysicalFrame::containing_address(self.address())
    }

    /// Sets the physical address of either the next page table this entry points
    /// to or the physical address of the physical frame if last level
    pub fn set_address(&mut self, addr: PhysicalAddress, flags: PageTableEntryFlags) {
        self.0 = addr.as_u64() | flags.bits();
    }

    pub fn flags(&self) -> PageTableEntryFlags {
        PageTableEntryFlags::from_bits_truncate(self.0)
    }

    pub fn set_flags(&mut self, flags: PageTableEntryFlags) {
        self.0 = flags.bits();
    }

    pub fn add_flags(&mut self, flags: PageTableEntryFlags) {
        self.0 = self.0 | flags.bits();
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }
}

#[repr(align(4096))]
#[repr(C)]
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

    pub unsafe fn initialize_empty_at_address(address: VirtualAddress) -> &'static mut PageTable {
        assert!(
            address.as_u64() as usize % PageTable::SIZE == 0,
            "Address must be properly aligned"
        );
        ptr::write(address.as_mut_ptr(), PageTable::empty());
        &mut *address.as_mut_ptr()
    }

    pub unsafe fn at_address(address: VirtualAddress) -> &'static mut PageTable {
        unsafe { &mut *address.as_mut_ptr() }
    }

    pub fn as_u64(&mut self) -> u64 {
        self as *mut Self as u64
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

#[derive(Debug)]
pub enum MappingError {
    FrameAllocationFailed,
    PageAlreadyMapped,
}

#[derive(Debug)]
pub enum UnmappingError {
    // Given page not mapped to physical frame
    PageNotMapped,
}

// TODO: make unsafe to mark that these functions are inherently unsafe
// S = trait wide scope
pub trait Mapper<S: PageSize> {
    // A = method wide scope
    fn map_to<A>(
        &mut self,
        from: PhysicalFrame<S>,
        to: Page<S>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<TlbFlusher<S>, MappingError>
    where
        A: FrameAllocator<Size4KiB>;

    fn identity_map<A>(
        &mut self,
        frame: PhysicalFrame<S>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<TlbFlusher<S>, MappingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let page = Page::containing_address(VirtualAddress::new(frame.address.as_u64()));
        self.map_to(frame, page, flags, frame_allocator)
    }

    fn unmap(&mut self, page: Page<S>)
        -> Result<(PhysicalFrame<S>, TlbFlusher<S>), UnmappingError>;
}

pub trait MapperAllSizes: Mapper<Size4KiB> + Mapper<Size2MiB> {}

impl<T> MapperAllSizes for T where T: Mapper<Size4KiB> + Mapper<Size2MiB> {}

#[derive(Debug)]
pub enum TranslationError {
    NotMapped,
}

pub trait TranslatorAllSizes: Translator<Size4KiB> + Translator<Size2MiB> {}

impl<T> TranslatorAllSizes for T where T: Translator<Size4KiB> + Translator<Size2MiB> {}

/// Translates page to physical frame using page table
pub trait Translator<S: PageSize> {
    fn translate(
        &self,
        page: Page<S>,
    ) -> Result<(PhysicalFrame<S>, PageTableEntryFlags), TranslationError>;
}

#[must_use = "Page table changes must be flushed or ignored"]
pub struct TlbFlusher<S: PageSize>(Page<S>);

impl<S: PageSize> TlbFlusher<S> {
    pub fn new(page: Page<S>) -> Self {
        TlbFlusher(page)
    }

    pub fn flush(self) {
        instructions::flush_tlb(self.0.address())
    }

    pub fn ignore(self) {}
}
