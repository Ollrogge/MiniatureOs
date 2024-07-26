use super::{process::Process, scheduler::Scheduler};
use crate::{
    allocator::stack_allocator::Stack,
    error::KernelError,
    memory::{
        address_space::AddressSpace,
        region::VirtualMemoryRegion,
        virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    },
    serial_println,
};
use alloc::{boxed::Box, string::String, sync::Arc, vec, vec::Vec};
use core::{pin::Pin, ptr::NonNull, slice};
use util::{intrusive_linked_list::Linked, mpsc_queue::Links, mutex::Mutex};
use x86_64::register::RFlags;
pub type ThreadEntryFunc = extern "C" fn() -> !;

#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct ThreadRegisterState {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rsp: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    rflags: u64,
    rip: u64,
}

#[derive(Clone, Copy)]
pub enum ThreadRunState {
    Ready,
    Running,
    Blocked,
    Finished,
}

pub enum ThreadPriority {
    Idle,
    Low,
    Normal,
    High,
    Max,
}
use core::ptr;

pub struct Thread {
    name: String,
    pub process: Arc<Mutex<Process>>,
    stack: VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>,
    // Ensure that the address always remains the same. Else when e.g. we get a
    // reference to the value and then push the thread into a vector, the reference
    // to the last_stack ptr will become invalid.
    // useful since we change this address in assembly code
    last_stack_ptr: Pin<Box<u64>>,
    state: ThreadRunState,
    priority: ThreadPriority,
}

pub extern "C" fn leave_thread() -> ! {
    unsafe { Scheduler::the().finish_current_thread() };
}

impl Thread {
    pub fn new<N>(
        name: N,
        process: Arc<Mutex<Process>>,
        stack: VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>,
        entry_point: ThreadEntryFunc,
        priority: ThreadPriority,
    ) -> Self
    where
        N: Into<String>,
    {
        let mut thread = Self {
            name: name.into(),
            process,
            stack,
            // will be set when stack of thread is setup
            last_stack_ptr: Box::pin(0),
            state: ThreadRunState::Ready,
            priority,
        };

        unsafe { thread.setup_stack(entry_point) };

        thread
    }

    pub fn colonel_thread<N>(
        name: N,
        process: Arc<Mutex<Process>>,
        stack: VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>,
    ) -> Self
    where
        N: Into<String>,
    {
        Self {
            name: name.into(),
            process,
            stack,
            last_stack_ptr: Box::pin(0),
            state: ThreadRunState::Ready,
            priority: ThreadPriority::Normal,
        }
    }

    unsafe fn setup_stack(&mut self, entry_point: ThreadEntryFunc) {
        let stack_slice: &mut [u8] =
            slice::from_raw_parts_mut(self.stack.start().as_mut_ptr(), self.stack.size());

        stack_slice.fill(0xff);
        let mut stack_writer = StackWriter::new(stack_slice);

        // stack end marker
        stack_writer.write_u64(0xDEADCAFEBEEFBABEu64);
        stack_writer.write_u64(leave_thread as *const () as u64);

        // points to the start of the thread_register_state
        let rsp = stack_writer.current_rsp() - size_of::<ThreadRegisterState>() as u64;
        serial_println!(
            "Thread stack_ptr: {:#x}, entry_point: {:#x}",
            rsp,
            entry_point as u64
        );

        stack_writer.write_thread_register_state(ThreadRegisterState {
            rsp,
            rbp: rsp,
            rip: entry_point as u64,
            rflags: (RFlags::IOPL_LOW | RFlags::INTERRUPT_FLAG).bits(),
            ..Default::default()
        });

        self.last_stack_ptr = Box::pin(rsp);
    }

    pub fn set_state(&mut self, state: ThreadRunState) {
        self.state = state;
    }

    pub fn cr3(&self) -> u64 {
        self.process.as_ref().lock().address_space().cr3()
    }

    pub fn last_stack_ptr_mut(&mut self) -> &mut u64 {
        &mut self.last_stack_ptr
    }

    pub fn last_stack_ptr(&self) -> u64 {
        *self.last_stack_ptr
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn finalize(&mut self) -> Result<(), KernelError> {
        Ok(())
    }
}

struct StackWriter<'a> {
    i: usize,
    data: &'a mut [u8],
}

impl<'a> StackWriter<'a> {
    fn new(data: &'a mut [u8]) -> Self {
        Self {
            i: data.len(),
            data,
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.advance_qword();
        let data = value.to_ne_bytes();
        let len = data.len();
        self.data[self.i..self.i + len].copy_from_slice(&data[..]);
    }

    fn write_thread_register_state(&mut self, val: ThreadRegisterState) {
        self.advance_n(size_of::<ThreadRegisterState>());
        let data = unsafe {
            core::mem::transmute::<ThreadRegisterState, [u8; size_of::<ThreadRegisterState>()]>(val)
        };
        let len = data.len();
        self.data[self.i..self.i + len].copy_from_slice(&data[..]);
    }

    fn advance_qword(&mut self) {
        self.advance_n(size_of::<u64>());
    }

    fn advance_n(&mut self, n: usize) {
        self.i -= n;
    }

    fn current_rsp(&mut self) -> u64 {
        if self.i == self.data.len() {
            &self.data[self.i - 1] as *const _ as u64 + 1
        } else {
            &self.data[self.i] as *const _ as u64
        }
        //self.data.as_ptr() as u64
    }
}
