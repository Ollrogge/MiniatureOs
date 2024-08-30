/// Simple semaphore implementation. Fair if using `acquire`. Potentially
/// unfair if using `try_acquire`
///
use core::{
    fmt::Display,
    ops::Drop,
    sync::atomic::{
        AtomicBool, AtomicUsize,
        Ordering::{self, AcqRel, Acquire, Relaxed, Release},
    },
};

#[derive(Debug, PartialEq)]
pub enum SemaphoreError {
    Closed,
    InvalidPermits,
    WouldBlock,
}

impl core::error::Error for SemaphoreError {}

impl Display for SemaphoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            SemaphoreError::Closed => write!(f, "Closed"),
            SemaphoreError::InvalidPermits => write!(f, "Invalid permits"),
            SemaphoreError::WouldBlock => write!(f, "Would block"),
        }
    }
}

pub struct Permit<'sem> {
    permits: usize,
    semaphore: &'sem Semaphore,
}

impl<'sem> Drop for Permit<'sem> {
    fn drop(&mut self) {
        self.semaphore.add_permits(self.permits)
    }
}

pub struct Semaphore {
    permits: AtomicUsize,
    closed: AtomicBool,
    max_permits: usize,
    waiter_idx: AtomicUsize,
    current_waiter_idx: AtomicUsize,
}

impl Semaphore {
    pub fn new(max_permits: usize) -> Self {
        Self {
            permits: AtomicUsize::new(max_permits),
            closed: AtomicBool::new(false),
            max_permits,
            waiter_idx: AtomicUsize::new(0),
            current_waiter_idx: AtomicUsize::new(0),
        }
    }

    fn acquire_spin(&self, permits_required: usize) -> Result<(), SemaphoreError> {
        if permits_required > self.max_permits {
            return Err(SemaphoreError::InvalidPermits);
        }
        let turn = self.waiter_idx.fetch_add(1, Acquire);
        loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(SemaphoreError::Closed);
            }

            if self.current_waiter_idx.load(Acquire) == turn {
                let current_permits = self.permits.load(Acquire);
                if current_permits >= permits_required {
                    if self
                        .permits
                        .compare_exchange(
                            current_permits,
                            current_permits - permits_required,
                            Ordering::AcqRel,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        self.current_waiter_idx.fetch_add(1, Release);
                        // Successfully acquired the permits
                        return Ok(());
                    }
                }
            }

            core::hint::spin_loop();
        }
    }

    // Called when semaphore can be deallocated. No new permits can be issued.
    // All waiters are notified
    pub fn close(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.permits.store(self.max_permits, Release);
    }

    pub fn acquire(&self, permits: usize) -> Result<Permit, SemaphoreError> {
        self.acquire_spin(permits)?;

        Ok(Permit {
            permits,
            semaphore: &self,
        })
    }

    /// This method is not fair since the caller might obtain a permit immediately
    /// even though there are other threads already waiting.
    pub fn try_acquire(&self, permits_required: usize) -> Result<Permit, SemaphoreError> {
        if permits_required > self.max_permits {
            return Err(SemaphoreError::InvalidPermits);
        }

        if self.closed.load(Ordering::Acquire) {
            return Err(SemaphoreError::Closed);
        }

        let current_permits = self.permits.load(Acquire);
        if current_permits >= permits_required {
            if self
                .permits
                .compare_exchange(
                    current_permits,
                    current_permits - permits_required,
                    AcqRel,
                    Relaxed,
                )
                .is_ok()
            {
                return Ok(Permit {
                    permits: permits_required,
                    semaphore: &self,
                });
            }
        }

        Err(SemaphoreError::WouldBlock)
    }

    pub fn add_permits(&self, permits: usize) {
        self.permits.fetch_add(permits, Release);
    }

    pub fn is_close(&self) -> bool {
        self.closed.load(Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    extern crate std;
    use alloc::vec;
    use std::{println, sync::Arc, thread, time::Duration};
    #[test]
    fn basic_test() {
        let sem = Arc::new(Semaphore::new(5));
        let mut handles = vec![];

        for i in 0..10 {
            let sem_clone = sem.clone();
            let handle = thread::spawn(move || {
                println!("Thread {} enter", i);
                let permit = if i == 5 {
                    sem_clone.acquire(5).unwrap()
                } else {
                    sem_clone.acquire(1).unwrap()
                };

                println!("Thread {} entered critical section", i);
                thread::sleep(Duration::from_millis(1000));
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn try_acquire_test() {
        let sem = Arc::new(Semaphore::new(1));

        let sem_clone = sem.clone();
        let handle = thread::spawn(move || {
            sem_clone.acquire(1).unwrap();

            thread::sleep(Duration::from_secs(1));
        });

        // make sure thread has spawned
        thread::sleep(Duration::from_millis(100));

        assert!(matches!(
            sem.try_acquire(1),
            Err(SemaphoreError::WouldBlock)
        ));

        handle.join().expect("Thread panicked");
    }
}
