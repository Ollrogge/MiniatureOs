use crate::{
    error::KernelError,
    multitasking::{
        process,
        scheduler::Scheduler,
        thread::{Thread, ThreadPriority},
    },
    serial_println,
};
use alloc::{string::String, vec::Vec};
use x86_64::instructions::hlt;

pub fn spawn_idle_thread() -> Result<(), KernelError> {
    process::spawn_kernel_thread("Idle", idle_thread_func, ThreadPriority::Idle)
}

extern "C" fn idle_thread_func() -> ! {
    serial_println!("Idle thread enter");
    loop {
        hlt();
    }
}

pub fn spawn_finalizer_thread() -> Result<(), KernelError> {
    process::spawn_kernel_thread("Finalizer", finializer_thread_func, ThreadPriority::Low)
}

extern "C" fn finializer_thread_func() -> ! {
    serial_println!("Finalizer thread enter");
    loop {
        let mut work = Vec::new();
        {
            let mut dying_threads = unsafe { Scheduler::the().dying_threads() };
            for _ in 0..3 {
                if let Some(t) = dying_threads.pop_front() {
                    work.push(t);
                } else {
                    break;
                }
            }
        }

        for mut thread in work {
            let res = thread.finalize();
            if let Err(err) = res {
                serial_println!("Failed to finialize thread: {}", err);
            }

            drop(thread)
        }

        hlt();
    }
}
