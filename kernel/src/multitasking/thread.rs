use super::process::Process;
use crate::allocator::stack_allocator::Stack;
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

#[derive(Clone)]
pub struct Thread {
    pub process: Arc<Mutex<Process>>,
    state: ThreadState,
    stack: VirtualRange,
}

impl Thread {
    pub fn new(
        process: Arc<Mutex<Process>>,
        stack: VirtualRange,
        entry_point: ThreadEntryPoint,
    ) -> Self {
        Self {
            process,
            state: ThreadState::Ready,
            stack,
        }
    }

    pub fn stack_top(&mut self) -> u64 {
        self.stack.top().as_u64()
    }

    pub unsafe fn stack_top_ptr(&mut self) -> *mut u64 {
        self.stack.top().inner_as_mut_ptr()
    }

    pub fn cr3(&self) -> PhysicalAddress {
        self.process.lock().cr3()
    }

    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }
}
