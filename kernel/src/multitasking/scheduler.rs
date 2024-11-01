use super::{
    process::Process,
    thread::{Thread, ThreadRunState},
};
use crate::serial_println;
use alloc::{boxed::Box, collections::VecDeque};
use core::{
    arch::asm,
    arch::naked_asm,
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::{self, addr_of_mut, NonNull},
    sync::atomic::{AtomicBool, Ordering},
};
use util::{
    intrusive_linked_list::Linked,
    mpsc_queue::{Links, MpscQueue},
    mutex::{Mutex, MutexGuard},
};
use x86_64::instructions::hlt;

// Wrapper around thread to fulfill mpsc's need for a stub entry
#[derive(Default)]
pub struct ThreadEntry {
    pub links: Links<ThreadEntry>,
    inner: Option<Thread>,
}

impl Unpin for ThreadEntry {}

impl ThreadEntry {
    pub fn new(thread: Thread) -> Pin<Box<Self>> {
        Box::pin(Self {
            links: Links::new(),
            inner: Some(thread),
        })
    }

    pub const fn new_const() -> Self {
        Self {
            links: Links::new(),
            inner: None,
        }
    }
}

impl Deref for ThreadEntry {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl DerefMut for ThreadEntry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("ThreadEntry inner is None")
    }
}

unsafe impl Linked<Links<ThreadEntry>> for ThreadEntry {
    type Handle = Pin<Box<Self>>;

    fn into_ptr(handle: Self::Handle) -> NonNull<ThreadEntry> {
        unsafe { NonNull::from(Box::leak(Pin::into_inner_unchecked(handle))) }
    }

    unsafe fn from_ptr(ptr: NonNull<Self>) -> Self::Handle {
        Pin::new_unchecked(Box::from_raw(ptr.as_ptr()))
    }

    unsafe fn links(target: NonNull<Self>) -> NonNull<Links<Self>> {
        let links = ptr::addr_of_mut!((*target.as_ptr()).links);

        NonNull::new_unchecked(links)
    }
}

static THREAD_ENTRY_STUB: ThreadEntry = ThreadEntry::new_const();

static mut SCHEDULER: Scheduler = {
    Scheduler {
        ready_threads: VecDeque::new(),
        dying_threads: unsafe { MpscQueue::new_with_static_stub(&THREAD_ENTRY_STUB) },
        running_thread: None,
        running_thread_is_finished: AtomicBool::new(false),
    }
};

pub struct Scheduler {
    ready_threads: VecDeque<Thread>,
    pub dying_threads: MpscQueue<ThreadEntry>,
    running_thread: Option<Thread>,
    running_thread_is_finished: AtomicBool,
}

pub fn schedule() {
    unsafe { Scheduler::the().schedule() }
}

impl Scheduler {
    pub fn add_thread(&mut self, thread: Thread) {
        self.ready_threads.push_back(thread);
    }

    pub fn finish_current_thread(&mut self) -> ! {
        self.running_thread_is_finished
            .store(true, Ordering::Relaxed);

        // Trigger scheduling
        loop {
            hlt();
        }
    }

    pub fn init(mut thread: Thread) {
        let scheduler = unsafe { Self::the() };
        thread.set_state(ThreadRunState::Running);
        scheduler._init(thread);
    }

    pub fn _init(&mut self, thread: Thread) {
        self.running_thread = Some(thread);
    }

    pub unsafe fn the() -> &'static mut Scheduler {
        &mut *addr_of_mut!(SCHEDULER)
    }

    pub fn schedule(&mut self) {
        if let Some(new_thread) = self.ready_threads.pop_front() {
            let mut old_thread = self.running_thread.take().unwrap();

            let old_cr3 = old_thread.cr3();
            let new_cr3 = new_thread.cr3();

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

            if self.running_thread_is_finished.load(Ordering::SeqCst) {
                self.dying_threads.enqueue(ThreadEntry::new(old_thread));
                self.running_thread_is_finished
                    .store(false, Ordering::Relaxed);
            } else {
                self.ready_threads.push_back(old_thread);
            }

            self.running_thread = Some(new_thread);

            unsafe { task_switch(old_rsp, new_rsp, old_cr3, new_cr3) };
        }
    }

    pub fn current_thread(&self) -> &Thread {
        self.running_thread.as_ref().unwrap()
    }

    pub fn current_thread_mut(&mut self) -> &mut Thread {
        self.running_thread.as_mut().unwrap()
    }
}

macro_rules! save_state {
    () => {
        "pushfq; push rax; push rcx; push rdx; push rbx; sub rsp, 8; push rbp; push rsi; push rdi; push r8; push r9; push r10; push r11; push r12; push r13; push r14; push r15"
    };
}

// skip rsp because we cant pop it as this would corrupt the stack layout
macro_rules! restore_state {
    () => {
        "pop r15; pop r14; pop r13; pop r12; pop r11; pop r10; pop r9; pop r8; pop rdi; pop rsi; pop rbp; add rsp, 8; pop rbx; pop rdx; pop rcx; pop rax; popfq"
    };
}

#[naked]
unsafe extern "C" fn task_switch(old_rsp: *mut u64, new_rsp: u64, old_cr3: u64, new_cr3: u64) {
    naked_asm!(
        save_state!(),
        "mov [rdi], rsp",
        "mov rsp, rsi",
        "cmp rdx, rcx",
        "je 2f",
        "mov cr3, rcx",
        "2:",
        restore_state!(),
        "ret",
    )
}
