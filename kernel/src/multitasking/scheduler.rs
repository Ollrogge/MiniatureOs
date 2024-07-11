use super::{
    process::Process,
    thread::{Thread, ThreadState},
};
use crate::{
    allocator::stack_allocator::Stack, memory::virtual_memory_object::VirtualMemoryObject,
};
use alloc::{collections::VecDeque, string::String, sync::Arc, vec::Vec};
use core::arch::asm;
use lazy_static::lazy_static;
use util::mutex::Mutex;
use x86_64::memory::{Address, PhysicalAddress, VirtualAddress};

lazy_static! {
    static ref SCHEDULER: Mutex<Scheduler> = {
        Mutex::new(Scheduler {
            ready_threads: VecDeque::new(),
            running_thread: None,
        })
    };
}

pub fn init(mut thread: Thread) {
    let mut scheduler = SCHEDULER.lock();
    thread.set_state(ThreadState::Running);
    scheduler.init(thread);
}

pub struct Scheduler {
    ready_threads: VecDeque<Thread>,
    running_thread: Option<Thread>,
}

pub fn the() -> &'static Mutex<Scheduler> {
    &SCHEDULER
}

impl Scheduler {
    pub fn add_thread(&mut self, thread: Thread) {
        self.ready_threads.push_back(thread);
    }

    pub fn init(&mut self, thread: Thread) {
        self.running_thread = Some(thread);
    }

    pub fn schedule(&mut self) {
        if let Some(new_thread) = self.ready_threads.pop_front() {
            let old = self.running_thread.take().unwrap();
            let old_cr3 = old.address_space().cr3;

            let new_cr3 = new_thread.address_space().cr3;

            let new_rsp = new_thread.last_stack_ptr().as_u64();
            let old_rsp = old.last_stack_ptr().as_mut_ptr();

            self.ready_threads.push_back(old);
            self.running_thread = Some(new_thread);
            unsafe { task_switch(old_rsp, new_rsp, old_cr3, new_cr3) };
        }
    }

    pub fn current_process(&self) -> &Arc<Mutex<Process>> {
        &self.running_thread.as_ref().unwrap().process
    }
}

macro_rules! save_state {
    () => {
        "pushfq; push rax; push rcx; push rdx; push rbx; push rbp; push rsi; push rdi; push r8; push r9; push r10; push r11; push r12; push r13; push r14; push r15"
    };
}

macro_rules! restore_state {
    () => {
        "pop r15; pop r14; pop r13; pop r12; pop r11; pop r10; pop r9; pop r8; pop rdi; pop rsi; pop rbp; pop rbx; pop rdx; pop rcx; pop rax; popfq"
    };
}

unsafe extern "C" fn task_switch(old_rsp: *mut u64, new_rsp: u64, old_cr3: u64, new_cr3: u64) {
    asm!(
        save_state!(),
        "mov [{0}], rsp",
        "mov rsp, {1}",
        "cmp {2}, {3}",
        "je 1f",
        "mov cr3, {3}",
        "1:",
        restore_state!(),
        "ret",
        in(reg) old_rsp,
        in(reg) new_rsp,
        in(reg) old_cr3,
        in(reg) new_cr3,
        options(noreturn)
    )
}
