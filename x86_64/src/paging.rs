use crate::{
    frame_allocator::FrameAllocator,
    memory::{Address, Page, PageSize, PhysicalAddress, PhysicalFrame, Size4KiB, VirtualAddress},
};
use bit_field::BitField;
use bitflags::bitflags;
use core::{
    ops::{Index, IndexMut},
    ptr,
    result::Result,
    slice,
};

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

    pub fn physical_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0.get_bits(12..48) << 12)
    }

    pub fn physical_frame(&self) -> PhysicalFrame {
        PhysicalFrame::containing_address(self.physical_address())
    }

    pub fn set_frame(&mut self, frame: PhysicalFrame, flags: PageTableEntryFlags) {
        self.0 = frame.address.as_u64() | flags.bits();
    }

    pub fn flags(&self) -> PageTableEntryFlags {
        PageTableEntryFlags::from_bits_truncate(self.0)
    }

    pub fn set_flags(&mut self, flags: PageTableEntryFlags) {
        self.0 = self.0 | flags.bits();
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

    pub fn initialize_empty_at_address(address: VirtualAddress) -> &'static mut PageTable {
        assert!(
            address.as_u64() as usize % PageTable::SIZE == 0,
            "Address must be properly aligned"
        );
        unsafe {
            ptr::write(address.as_mut_ptr(), PageTable::empty());
        }
        unsafe { &mut *address.as_mut_ptr() }
    }

    pub fn at_address(address: VirtualAddress) -> &'static mut PageTable {
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

/// Assumes that all page table related data is 1:1 mapped.
pub struct FourLevelPageTable<'a> {
    pml4t: &'a mut PageTable,
    walker: PageTableWalker,
}

impl<'a> FourLevelPageTable<'a> {
    pub fn new(pml4t: &'a mut PageTable) -> Self {
        Self {
            pml4t,
            walker: PageTableWalker {},
        }
    }
}

/// This struct only exists to avoid borrowing self twice in the map_to func
struct PageTableWalker;

impl PageTableWalker {
    /// Allocates pagetable or returns it if already existing
    // assumes 1:1 mapping for physical frames holding pagetable data
    // using virtual addresses here because we are in long mode and all memory accesses
    // are based on virtual memory. So just to make it explicit
    pub fn get_or_allocate_pagetable<'a, A>(
        &self,
        pagetable_entry: &'a mut PageTableEntry,
        flags: PageTableEntryFlags,
        allocator: &mut A,
    ) -> Option<&'a mut PageTable>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let table = if pagetable_entry.is_unused() {
            let frame = allocator.allocate_frame()?;
            pagetable_entry.set_frame(frame, flags);

            let virtual_address = VirtualAddress::new(frame.address.as_u64());
            let table = PageTable::initialize_empty_at_address(virtual_address);
            table
        } else {
            if !flags.is_empty() && !pagetable_entry.flags().contains(flags) {
                pagetable_entry.set_flags(pagetable_entry.flags() | flags);
            }
            // 1:1
            let virtual_address =
                VirtualAddress::new(pagetable_entry.physical_frame().address.as_u64());
            PageTable::at_address(virtual_address)
        };

        Some(table)
    }

    pub fn get_pagetable<'a>(&self, pagetable_entry: &'a PageTableEntry) -> Option<&'a PageTable> {
        if !pagetable_entry.is_unused() {
            let virtual_address =
                VirtualAddress::new(pagetable_entry.physical_frame().address.as_u64());
            Some(PageTable::at_address(virtual_address))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum MappingError {
    FrameAllocationFailed,
    PageAlreadyMapped,
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
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<S>;

    fn identity_map<A>(
        &mut self,
        frame: PhysicalFrame<S>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<S>,
    {
        let page = Page::containing_address(VirtualAddress::new(frame.address.as_u64()));
        self.map_to(frame, page, flags, frame_allocator)
    }
}

impl<'a> Mapper<Size4KiB> for FourLevelPageTable<'a> {
    fn map_to<A>(
        &mut self,
        frame: PhysicalFrame<Size4KiB>,
        page: Page<Size4KiB>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<(), MappingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let parent_flags = PageTableEntryFlags::PRESENT
            | PageTableEntryFlags::WRITABLE
            | PageTableEntryFlags::USER_ACCESSIBLE;
        let l4 = &mut self.pml4t;
        let l3 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l4[page.address.l4_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l2 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l3[page.address.l3_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l1 = self
            .walker
            .get_or_allocate_pagetable(
                &mut l2[page.address.l2_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;

        let pte = &mut l1[page.address.l1_index()];

        if pte.is_present() {
            Err(MappingError::PageAlreadyMapped)
        } else {
            pte.set_frame(frame, flags);
            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum TranslationError {
    NotMapped,
}

/// Translates page to physical frame using page table
pub trait Translator<S: PageSize> {
    fn translate(&self, page: Page<S>) -> Result<PhysicalFrame, TranslationError>;
}

impl<'a> Translator<Size4KiB> for FourLevelPageTable<'a> {
    fn translate(&self, page: Page<Size4KiB>) -> Result<PhysicalFrame, TranslationError> {
        let l4 = &self.pml4t;
        let l3 = self
            .walker
            .get_pagetable(&l4[page.address.l4_index()])
            .ok_or(TranslationError::NotMapped)?;
        let l2 = self
            .walker
            .get_pagetable(&l3[page.address.l3_index()])
            .ok_or(TranslationError::NotMapped)?;
        let l1 = self
            .walker
            .get_pagetable(&l2[page.address.l2_index()])
            .ok_or(TranslationError::NotMapped)?;

        let pte = &l1[page.address.l1_index()];

        if pte.is_present() {
            Ok(pte.physical_frame())
        } else {
            Err(TranslationError::NotMapped)
        }
    }
}
