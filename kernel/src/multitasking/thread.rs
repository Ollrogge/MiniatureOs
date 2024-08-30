use super::{process::Process, scheduler::Scheduler};
use crate::{
    allocator::stack_allocator::Stack,
    error::KernelError,
    memory::{
        address_space::AddressSpace,
        manager::MemoryManager,
        region::{RegionType, VirtualMemoryRegion},
        virtual_memory_object::{MemoryBackedVirtualMemoryObject, VirtualMemoryObject},
    },
    multitasking::process::ThreadId,
    serial_println,
};
use alloc::{boxed::Box, string::String, sync::Arc, vec, vec::Vec};
use core::{future::poll_fn, iter::zip, pin::Pin, ptr::NonNull, slice};
use util::{intrusive_linked_list::Linked, mpsc_queue::Links, mutex::Mutex};
use x86_64::{
    interrupts::PageFaultErrorCode, memory::VirtualAddress, paging::PageTableEntryFlags,
    register::RFlags,
};

pub type ThreadEntryFunc = extern "C" fn();

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

pub struct Thread {
    id: ThreadId,
    name: String,
    pub process: Arc<Mutex<Process>>,
    mem_regions: Vec<VirtualMemoryRegion>,
    // Ensure that the address always remains the same. Else when e.g. we get a
    // reference to the value and then push the thread into a vector, the reference
    // to the last_stack ptr will become invalid.
    // useful since we change this address in assembly code
    last_stack_ptr: Pin<Box<u64>>,
    state: ThreadRunState,
    priority: ThreadPriority,
    entry_func: ThreadEntryFunc,
}

pub extern "C" fn leave_thread() -> ! {
    unsafe { Scheduler::the().finish_current_thread() };
}

impl Thread {
    pub fn new<N>(
        id: ThreadId,
        name: N,
        // TODO: Rwlock
        process: Arc<Mutex<Process>>,
        stack: VirtualMemoryRegion,
        priority: ThreadPriority,
        entry: ThreadEntryFunc,
    ) -> Self
    where
        N: Into<String>,
    {
        let mut thread = Self {
            id,
            name: name.into(),
            process,
            mem_regions: vec![stack],
            // will be set when stack of thread is setup
            last_stack_ptr: Box::pin(0),
            state: ThreadRunState::Ready,
            priority,
            entry_func: entry,
        };

        thread
    }

    // colonel thread = thread created from the code that was running on kernel
    // entry and initialized everything
    pub fn colonel_thread<N>(
        id: ThreadId,
        name: N,
        process: Arc<Mutex<Process>>,
        stack: VirtualMemoryRegion,
    ) -> Self
    where
        N: Into<String>,
    {
        Self {
            id,
            name: name.into(),
            process,
            mem_regions: vec![stack],
            last_stack_ptr: Box::pin(0),
            state: ThreadRunState::Ready,
            priority: ThreadPriority::Normal,
            entry_func: unsafe { core::mem::transmute(0 as usize) },
        }
    }

    pub unsafe fn setup_stack(&mut self) {
        let stack = self
            .mem_regions
            .iter()
            .find(|x| x.typ() == RegionType::Stack)
            .unwrap();

        let stack_slice: &mut [u8] =
            slice::from_raw_parts_mut(stack.start().as_mut_ptr(), stack.size());

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
            self.entry_func as u64
        );

        stack_writer.write_thread_register_state(ThreadRegisterState {
            rsp,
            rbp: rsp,
            rip: self.entry_func as u64,
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

    pub fn handle_page_fault(
        &mut self,
        address: VirtualAddress,
        error: PageFaultErrorCode,
    ) -> Result<(), KernelError> {
        let region = self.mem_regions.iter_mut().find(|x| x.contains(address));

        if !error.is_non_present_fault() {
            serial_println!("Unhandled page fault: {:?}", error);
            return Err(KernelError::Other);
        }

        let mut process = self.process.lock();

        if let Some(region) = region {
            // TODO: check region backing type (file vs memory)?

            // TODO: implement reserving mechanism such that we never encounter the case
            // that a lazily allocated thread that is running is unable to allocate
            // memory
            let frames = MemoryManager::the()
                .lock()
                .try_allocate_frames(region.page_range().len())?;

            let access_flags: PageTableEntryFlags =
                Into::<PageTableEntryFlags>::into(region.access_flags())
                    | PageTableEntryFlags::PRESENT;

            // unmap pages to then remap them with frames
            for page in region.page_range().iter().skip(1) {
                let (_, flusher) = process.address_space().unmap(page)?;
                flusher.ignore();
            }
            // skip guard page, index 0 since stack grows downwards => lowest address is end of stack
            for (frame, page) in zip(frames.iter(), region.page_range().iter().skip(1)) {
                unsafe {
                    process
                        .address_space()
                        .map_to(frame.clone(), page, access_flags)?
                        .flush();
                }
            }

            drop(process);

            if region.typ() == RegionType::Stack {
                unsafe { self.setup_stack() };
            }

            Ok(())
        } else {
            // TODO: handle page faults for regions managed by the process
            // for now we error out
            Err(KernelError::Other)
        }
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
