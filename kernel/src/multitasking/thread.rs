use super::process::ProcessControlBlock;
use crate::allocator::stack_allocator::Stack;
use alloc::sync::Arc;
use util::mutex::Mutex;
use x86_64::memory::{Address, PhysicalAddress, VirtualAddress};

#[derive(Clone, Copy)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Finished,
}

#[derive(Clone)]
pub struct ThreadControlBlock {
    pub process: Arc<Mutex<ProcessControlBlock>>,
    state: ThreadState,
    stack: Stack,
}

impl ThreadControlBlock {
    pub fn new(process: Arc<Mutex<ProcessControlBlock>>, stack: Stack) -> Self {
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
