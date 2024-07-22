#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
use api::BootInfo;
use error::KernelError;
extern crate alloc;
use core::{iter::Copied, ops::DerefMut};
use util::mutex::{Mutex, MutexGuard};
use x86_64::{
    gdt::GlobalDescriptorTable,
    memory::{Address, MemoryRegion, PhysicalMemoryRegion},
    paging::{
        bump_frame_allocator::BumpFrameAllocator,
        linked_list_frame_allocator::LinkedListFrameAllocator,
        offset_page_table::{OffsetPageTable, PhysicalOffset},
    },
};

pub mod allocator;
pub mod error;
pub mod housekeeping_threads;
pub mod interrupts;
pub mod memory;
pub mod multitasking;
pub mod paging;
pub mod qemu;
pub mod serial;
pub mod vga;

static GLOBAL_DATA: Mutex<GlobalData> = Mutex::new(GlobalData::new());

pub struct GlobalData {
    physical_memory_offset: usize,
}

impl GlobalData {
    pub const fn new() -> Self {
        Self {
            physical_memory_offset: 0,
        }
    }

    pub fn the() -> MutexGuard<'static, GlobalData> {
        GLOBAL_DATA.lock()
    }

    pub fn init(&mut self, physical_memory_offset: usize) {
        self.physical_memory_offset = physical_memory_offset
    }

    pub fn physical_memory_offset(&self) -> usize {
        self.physical_memory_offset
    }
}

use memory::manager::MemoryManager;

pub fn kernel_init(boot_info: &'static BootInfo) -> Result<(), KernelError> {
    MemoryManager::the().lock().init(boot_info)?;
    interrupts::init();

    GlobalData::the().init(boot_info.physical_memory_offset);

    Ok(())
}
