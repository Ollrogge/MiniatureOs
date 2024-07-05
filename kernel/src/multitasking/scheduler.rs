use super::{
    process::ProcessControlBlock,
    thread::{ThreadControlBlock, ThreadState},
};
use crate::allocator::stack_allocator::Stack;
use alloc::{collections::VecDeque, string::String, sync::Arc, vec::Vec};
use core::arch::asm;
use lazy_static::lazy_static;
use util::mutex::Mutex;
use x86_64::memory::{Address, PhysicalAddress, VirtualAddress};

lazy_static! {
    static ref SCHEDULER: Mutex<Scheduler> = {
        let dummy_process = Arc::new(Mutex::new(ProcessControlBlock::new(
            String::new(),
            PhysicalAddress::new(0),
        )));
        let dummy_thread = ThreadControlBlock::new(dummy_process, Stack::default());

        Mutex::new(Scheduler {
            ready_threads: VecDeque::new(),
            processes: VecDeque::new(),
            running_thread: dummy_thread,
        })
    };
}

pub fn init(process: Arc<Mutex<ProcessControlBlock>>, mut thread: ThreadControlBlock) {
    let mut scheduler = SCHEDULER.lock();
    thread.set_state(ThreadState::Running);
    scheduler.running_thread = thread;
    scheduler.processes.push_back(process);
}

pub struct Scheduler {
    processes: VecDeque<Arc<Mutex<ProcessControlBlock>>>,
    ready_threads: VecDeque<ThreadControlBlock>,
    running_thread: ThreadControlBlock,
}

pub fn val() -> &'static Mutex<Scheduler> {
    &SCHEDULER
}

impl Scheduler {
    pub fn add_thread(&mut self, thread: ThreadControlBlock) {
        self.ready_threads.push_back(thread);
    }

    pub fn schedule(&mut self) {
        if let Some(new_thread) = self.ready_threads.pop_front() {
            let old = self.running_thread.clone();
            let old_cr3 = old.cr3();
            self.running_thread = new_thread;
            self.ready_threads.push_back(old);

            let new_cr3 = self.running_thread.cr3();

            let new_rsp = self.running_thread.stack_top();
            let old_rsp = unsafe { self.ready_threads.back_mut().unwrap().stack_top_ptr() };

            unsafe { task_switch(old_rsp, new_rsp, old_cr3.as_u64(), new_cr3.as_u64()) };
        }
    }

    pub fn current_process(&self) -> &Arc<Mutex<ProcessControlBlock>> {
        &self.running_thread.process
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
