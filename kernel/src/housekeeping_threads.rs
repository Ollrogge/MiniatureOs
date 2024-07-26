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
        //serial_println!("Idle");
    }
}

pub fn spawn_finalizer_thread() -> Result<(), KernelError> {
    process::spawn_kernel_thread("Finalizer", finializer_thread_func, ThreadPriority::Low)
}

extern "C" fn finializer_thread_func() -> ! {
    serial_println!("Finalizer thread enter");
    // Obtain exclusive comsumer right to the dying threads queue
    let consumer = unsafe {
        Scheduler::the()
            .dying_threads
            .try_consume()
            .expect("Finalizer thread was unable to become consumer for dying threads")
    };
    loop {
        for _ in 0..3 {
            if let Some(mut thread) = consumer.dequeue() {
                if let Err(err) = thread.finalize() {
                    serial_println!("Failed to finialze thread: {}", err);
                }
                drop(thread);
            } else {
                break;
            }
        }

        hlt();
        //serial_println!("Finalizer");
    }
}
