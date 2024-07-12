use super::{
    process::Process,
    thread::{Thread, ThreadRunState},
};
use crate::{
    allocator::stack_allocator::Stack, memory::virtual_memory_object::VirtualMemoryObject, println,
    serial_println,
};
use alloc::{boxed::Box, collections::VecDeque, string::String, sync::Arc, vec::Vec};
use core::{arch::asm, pin::Pin};
use lazy_static::lazy_static;
use util::mutex::Mutex;
use x86_64::memory::{Address, PhysicalAddress, VirtualAddress};

// Scheduler cant be protected by a mutex since it will not be dropped on task switch
static mut SCHEDULER: Scheduler = {
    Scheduler {
        ready_threads: VecDeque::new(),
        running_thread: None,
    }
};

pub struct Scheduler {
    ready_threads: VecDeque<Thread>,
    running_thread: Option<Thread>,
}

pub fn schedule() {
    unsafe { Scheduler::the().schedule() }
}

impl Scheduler {
    pub fn add_thread(&mut self, thread: Thread) {
        self.ready_threads.push_back(thread);
    }

    pub fn init(mut thread: Thread) {
        let scheduler = unsafe { Self::the() };
        thread.set_state(ThreadRunState::Running);
        scheduler._init(thread);
    }

    pub fn _init(&mut self, thread: Thread) {
        self.running_thread = Some(thread);
    }

    pub(crate) unsafe fn the() -> &'static mut Scheduler {
        &mut SCHEDULER
    }

    pub fn schedule(&mut self) {
        if let Some(new_thread) = self.ready_threads.pop_front() {
            let mut old_thread = self.running_thread.take().unwrap();

            let old_cr3 = old_thread.address_space().cr3;
            let new_cr3 = new_thread.address_space().cr3;

            let new_rsp = new_thread.last_stack_ptr();
            let old_rsp = old_thread.last_stack_ptr_mut() as *mut u64;

            /*
            serial_println!(
                "Schedule: old: {} rsp: {:#x}, new: {} rsp: {:#x}",
                old_thread.name(),
                old_rsp as u64,
                new_thread.name(),
                new_rsp,
            );
            */

            self.ready_threads.push_back(old_thread);
            self.running_thread = Some(new_thread);

            unsafe { task_switch(old_rsp, new_rsp, old_cr3, new_cr3) };
        }
    }

    pub fn current_process(&self) -> Arc<Mutex<Process>> {
        self.running_thread.as_ref().unwrap().process.clone()
    }
}

macro_rules! save_state {
    () => {
        "pushfq; push rax; push rcx; push rdx; push rbx; sub rsp, 8; push rbp; push rsi; push rdi; push r8; push r9; push r10; push r11; push r12; push r13; push r14; push r15"
    };
}

// skip rsp because we cant pop it since this would corrupt the stack layout
macro_rules! restore_state {
    () => {
        "pop r15; pop r14; pop r13; pop r12; pop r11; pop r10; pop r9; pop r8; pop rdi; pop rsi; pop rbp; add rsp, 8; pop rbx; pop rdx; pop rcx; pop rax; popfq"
    };
}

#[naked]
unsafe extern "C" fn task_switch(old_rsp: *mut u64, new_rsp: u64, old_cr3: u64, new_cr3: u64) {
    asm!(
        save_state!(),
        "mov [rdi], rsp",
        "mov rsp, rsi",
        "cmp rdx, rcx",
        "je 1f",
        "mov cr3, rcx",
        "1:",
        restore_state!(),
        "ret",
        options(noreturn)
    )
}
