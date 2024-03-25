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
use x86_64::{
    memory::{GIB, MIB},
    mutex::Mutex,
    paging::{PageTable, PageTableEntry, PageTableEntryFlags},
    println,
};

static PML4T: Mutex<PageTable> = Mutex::new(PageTable::empty());
static PDPT: Mutex<PageTable> = Mutex::new(PageTable::empty());
static PDT: Mutex<[PageTable; 10]> = Mutex::new([PageTable::empty(); 10]);

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
// map 10 GiB in total
fn create_mappings() {
    // can be sure that the addresses of the tables work since stage3 is mapped at 1MiB
    let flags = PageTableEntryFlags::WRITABLE | PageTableEntryFlags::PRESENT;
    let mut l4 = PML4T.lock();
    let mut l3 = PDPT.lock();
    let mut l2 = PDT.lock();
    l4.entries[0] = PageTableEntry::new(l3.deref_mut().as_u64() | flags.bits());
    for (i, l2) in l2.iter_mut().enumerate() {
        l3.entries[i] = PageTableEntry::new(l2.as_u64() | flags.bits());
        // 1 l2 table = 1 GiB of Memory (512 * 2 MiB huge pages)
        let offset = u64::try_from(i).unwrap() * (1 * GIB as u64);
        for (j, entry) in l2.entries.iter_mut().enumerate() {
            // map huge pages
            *entry = PageTableEntry::new(
                (offset + u64::try_from(j).unwrap() * (2 * MIB as u64))
                    | flags.bits()
                    | PageTableEntryFlags::HUGE_PAGE.bits(),
            )
        }
    }
}

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
