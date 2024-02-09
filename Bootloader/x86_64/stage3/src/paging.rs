//! Physical address extension (PAE) paging = 4 levels
//!
//!
//! PML4T = page-map level-4 table
//! PDPT = page directory pointer table
//! PDT = page directory table
//! PT = page table
//!
//! PML4T -> PDPT -> PDT -> PT

use core::{arch::asm, borrow::BorrowMut, ops::DerefMut, slice};

use crate::mutex::Mutex;
use crate::println;
use bitflags::bitflags;

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

static PML4T: Mutex<PageTable> = Mutex::new(PageTable::empty());
static PDPT: Mutex<PageTable> = Mutex::new(PageTable::empty());
static PDT: Mutex<[PageTable; 10]> = Mutex::new([PageTable::empty(); 10]);

const TABLE_ENTRY_COUNT: usize = 512;
const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy)]
struct PageTableEntry(u64);

#[repr(align(4096))]
#[derive(Clone, Copy)]
struct PageTable {
    entries: [PageTableEntry; TABLE_ENTRY_COUNT],
}

impl PageTable {
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

pub fn init() {
    create_mappings();

    enable_paging();
}

// PML4T[0] -> PDPT.
// PDPT[0] -> PDT.
// PDT[0] -> PT.
// PT -> 0x00000000 - 0x00200000.
//
// Use huge pages to make the loading faster, else .bss is big which takes a lot of time
// to load into memory from FAT
fn create_mappings() {
    // can be sure that the addresses of the tables work since stage3 is mapped at 1MiB
    let flags = PageTableFlags::WRITABLE | PageTableFlags::PRESENT;
    let mut l4 = PML4T.lock();
    let mut l3 = PDPT.lock();
    let mut l2 = PDT.lock();
    l4.entries[0] = PageTableEntry((&mut *l3 as *mut PageTable as u64) | flags.bits());
    for (i, l2) in l2.iter_mut().enumerate() {
        l3.entries[i] = PageTableEntry((l2 as *mut PageTable as u64) | flags.bits());
        let offset = u64::try_from(i).unwrap() * 1024 * 1024 * 1024;
        for (j, entry) in l2.entries.iter_mut().enumerate() {
            // map huge pages
            *entry = PageTableEntry(
                (offset + u64::try_from(j).unwrap() * (2 * 1024 * 1024))
                    | flags.bits()
                    | PageTableFlags::HUGE_PAGE.bits(),
            )
        }
    }
}

/*
fn create_mappings() {
    // can be sure that the addresses of the tables work since stage3 is mapped at 1MiB
    let flags = PageTableFlags::WRITABLE | PageTableFlags::PRESENT;
    let mut l4 = PML4T.lock();
    let mut l3 = PDPT.lock();
    let mut l2 = PDT.lock();
    let mut l1 = PT.lock();
    l4.entries[0] = PageTableEntry((&mut *l3 as *mut PageTable as u64) | flags.bits());
    for i in 0..l2.len() {
        l3.entries[i] = PageTableEntry((&mut l2[i] as *mut PageTable as u64) | flags.bits());
    }
    for i in 0..l1.len() {
        l2[i / TABLE_ENTRY_COUNT].entries[i % TABLE_ENTRY_COUNT] =
            PageTableEntry((&mut l1[i] as *mut PageTable as u64) | flags.bits());
    }
    for (i, table) in l1.iter_mut().enumerate() {
        for (j, entry) in table.iter_mut().enumerate() {
            // 1:1 mapping to physical address
            let addr = ((i * TABLE_ENTRY_COUNT + j) * PAGE_SIZE) as u64;
            *entry = PageTableEntry(addr | flags.bits());
        }
    }
}
*/

fn enable_paging() {
    // load level 4 table pointer into cr3 register
    let l4 = PML4T.lock().deref_mut() as *mut PageTable as u32;
    unsafe { asm!("mov cr3, {0}", in(reg) l4) };

    // enable PAE-flag in cr4 (Physical Address Extension)
    unsafe { asm!("mov eax, cr4", "or eax, 1<<5", "mov cr4, eax", out("eax")_) };

    // set the long mode bit in the EFER MSR (model specific register)
    unsafe {
        asm!("mov ecx, 0xC0000080", "rdmsr", "or eax, 1 << 8", "wrmsr", out("eax") _, out("ecx")_)
    };

    // enable paging in the cr0 register
    unsafe { asm!("mov eax, cr0", "or eax, 1 << 31", "mov cr0, eax", out("eax")_) };
}
