use super::process::Process;
use crate::{
    allocator::stack_allocator::Stack,
    memory::{
        address_space::AddressSpace,
        region::VirtualMemoryRegion,
        virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    },
};
use alloc::{sync::Arc, vec, vec::Vec};
use util::mutex::Mutex;
use x86_64::memory::{Address, PhysicalAddress, VirtualAddress, VirtualRange, KIB};

pub type ThreadEntryPoint = extern "C" fn();

#[derive(Clone, Copy)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Finished,
}

pub struct Thread {
    pub process: Arc<Mutex<Process>>,
    stack: VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>,
    last_stack_ptr: VirtualAddress,
    state: ThreadState,
}

impl Thread {
    pub fn new(
        process: Arc<Mutex<Process>>,
        stack: VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>,
        last_stack_ptr: VirtualAddress,
    ) -> Self {
        Self {
            process,
            stack,
            last_stack_ptr,
            state: ThreadState::Ready,
        }
    }

    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    pub fn address_space(&self) -> AddressSpace {
        self.process.as_ref().lock().address_space()
    }

    pub fn last_stack_ptr(&self) -> VirtualAddress {
        self.last_stack_ptr
    }
}
