use bit_field::BitField;
use bitflags::{bitflags, Flags};
use core::borrow::BorrowMut;
use core::marker::PhantomData;
use core::ops::Add;
use core::ops::{Index, IndexMut};
use core::result::Result;
use core::slice;
use core::{clone, ptr};

use crate::frame_allocator::{self, FrameAllocator};
use crate::memory::{Address, Page, PageSize, PhysicalFrame, Size4KiB, VirtualAddress};

bitflags! {
    /// Possible flags for a page table entry.
    #[derive(Clone, Copy)]
    pub struct PageTableEntryFlags: u64 {
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

    pub fn is_present(&self) -> bool {
        (self.0 & PageTableEntryFlags::PRESENT.bits()) != 0
    }

    pub fn physical_address(&self) -> u64 {
        self.0.get_bits(12..48)
    }

    pub fn set_frame(&mut self, frame: PhysicalFrame, flags: PageTableEntryFlags) {
        self.0 = frame.address.as_u64() | flags.bits();
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

    pub fn initialize_empty_at_address(address: VirtualAddress) -> *mut PageTable {
        assert!(
            address.as_u64() as usize % PageTable::SIZE == 0,
            "Address must be properly aligned"
        );
        unsafe {
            ptr::copy_nonoverlapping(&PageTable::empty(), address.as_mut_ptr(), PageTable::SIZE);
            &mut *(address.as_u64() as *mut PageTable)
        }
    }

    pub fn at_address(address: VirtualAddress) -> *mut PageTable {
        unsafe { &mut *(address.as_mut_ptr() as *mut PageTable) }
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
    pub fn get_pagetable<'a, A>(
        &self,
        pagetable_entry: &'a mut PageTableEntry,
        flags: PageTableEntryFlags,
        allocator: &mut A,
    ) -> Option<&'a mut PageTable>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let table = if !pagetable_entry.is_present() {
            let frame = allocator.allocate_frame()?;
            pagetable_entry.set_frame(frame, flags);
            let virtual_address = VirtualAddress::new(frame.address.as_u64());
            let table = PageTable::initialize_empty_at_address(virtual_address);
            table
        } else {
            let virtual_address = VirtualAddress::new(pagetable_entry.physical_address());
            PageTable::at_address(virtual_address)
        };

        let table = unsafe { &mut *table };

        Some(table)
    }
}

/*
impl<'a, 'b> FourLevelPageTable<'a> {
    /// Gets the pagetable the passed entry points to or allocates a new table if the
    /// entry is not present
    pub fn get_pagetable<A>(
        &self,
        pagetable_entry: &'b mut PageTableEntry,
        allocator: &mut A,
    ) -> Option<PageTable>
    where
        A: FrameAllocator<Size4KiB>,
    {
        if !pagetable_entry.is_present() {
            let frame = allocator.allocate_frame()?;
            let mut virtual_address = VirtualAddress::new(frame.address.as_u64());
            let mut table = PageTable::initialize_empty_at_virtual_address(&mut virtual_address);
            *pagetable_entry = PageTableEntry::new(table.as_u64());
            Some(table)
        } else {
            Some(PageTable::at_address(VirtualAddress::new(
                pagetable_entry.physical_address(),
            )))
        }
        // if not present -> allocate, else return
    }
}
*/

#[derive(Debug)]
pub enum MappingError {
    FrameAllocationFailed,
    PageAlreadyMapped,
}

pub struct MappedPageTable<'a> {
    pml4t: &'a mut PageTable,
}

// S = trait wide scope
pub trait Mapper<S> {
    // A = method wide scope
    fn map_to<A>(
        &mut self,
        from: PhysicalFrame<S>,
        to: Page<S>,
        flags: PageTableEntryFlags,
        frame_allocator: &mut A,
    ) -> Result<(), MappingError>
    where
        S: PageSize,
        A: FrameAllocator<S>;
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
            .get_pagetable(
                &mut l4[page.address.l4_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l2 = self
            .walker
            .get_pagetable(
                &mut l3[page.address.l3_index()],
                parent_flags,
                frame_allocator,
            )
            .ok_or(MappingError::FrameAllocationFailed)?;
        let l1 = self
            .walker
            .get_pagetable(
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

/*
pub struct Mapper<'a, A, S> {
    allocator: &'a mut A,
    _marker: PhantomData<S>,
}

impl<'a, A, S> Mapper<'a, A, S>
where
    A: FrameAllocator<S>,
    S: PageSize,
{
    pub fn new(frame_allocator: &'a mut A) -> Self {
        Self {
            allocator: frame_allocator,
            _marker: PhantomData,
        }
    }
}
*/
