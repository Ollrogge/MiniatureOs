use super::{scheduler, thread::ThreadControlBlock};
use crate::allocator::stack_allocator::Stack;
use alloc::{string::String, sync::Arc};
use core::{
    arch::asm,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};
use util::mutex::Mutex;
use x86_64::{
    memory::{PhysicalAddress, VirtualAddress},
    register::Cr3,
};

/**
 * Each process requires a MemoryManager bound to the current address space
 * MemoryManager needs access to frameallocator and pagetable
*/

pub struct ProcessId(u64);

impl ProcessId {
    pub fn new() -> Self {
        static IDS: AtomicU64 = AtomicU64::new(0);
        Self(IDS.fetch_add(1, Relaxed))
    }
}

// TODO: add a memory manager which manages the whole memory for the process
// and has access to frame_allocator and page_table
pub struct ProcessControlBlock {
    id: ProcessId,
    name: String,
    cr3: PhysicalAddress,
}

pub fn init(initial_kernel_stack: Stack) {
    let (cr3, _) = Cr3::read();
    let process = Arc::new(Mutex::new(ProcessControlBlock::new(
        String::from("kernel_root"),
        cr3.address(),
    )));

    let thread = ThreadControlBlock::new(process.clone(), initial_kernel_stack);

    scheduler::init(process, thread);
}

impl ProcessControlBlock {
    pub fn new(name: String, cr3: PhysicalAddress) -> Self {
        Self {
            id: ProcessId::new(),
            name,
            cr3,
        }
    }

    pub fn cr3(&self) -> PhysicalAddress {
        self.cr3
    }
}

pub fn start_thread_in_current_process(name: String, func: extern "C" fn()) {
    let cur_process = scheduler::val().lock().current_process();

    //let thread = ThreadControlBlock::new(cur_process.clone());
}
