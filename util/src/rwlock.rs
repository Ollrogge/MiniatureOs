use crate::{
    mutex::Mutex,
    semaphore::{self, Permit, Semaphore},
};
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    usize,
};

pub struct RwLock<T> {
    state: Semaphore,
    value: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    pub fn new(val: T) -> Self {
        Self {
            state: Semaphore::new(usize::MAX),
            value: UnsafeCell::new(val),
        }
    }

    pub fn read(&self) -> Result<ReadGuard<'_, T>, ()> {
        let permit = self.state.acquire(1).map_err(|_| ())?;
        Ok(ReadGuard {
            rwlock: &self,
            permit,
        })
    }

    pub fn try_read(&self) -> Result<ReadGuard<'_, T>, ()> {
        let permit = self.state.try_acquire(1).map_err(|_| ())?;
        Ok(ReadGuard {
            rwlock: &self,
            permit,
        })
    }

    pub fn write(&self) -> Result<WriteGuard<'_, T>, ()> {
        let permit = self.state.acquire(usize::MAX).map_err(|_| ())?;
        Ok(WriteGuard {
            rwlock: &self,
            permit,
        })
    }

    pub fn try_write(&self) -> Result<WriteGuard<'_, T>, ()> {
        let permit = self.state.try_acquire(usize::MAX).map_err(|_| ())?;
        Ok(WriteGuard {
            rwlock: &self,
            permit,
        })
    }
}
unsafe impl<T> Sync for RwLock<T> where T: Send + Sync {}

pub struct ReadGuard<'a, T> {
    rwlock: &'a RwLock<T>,
    // ReadGuard owns the permit such that when it goes out of scope, the
    // permits will be added to the semaphore again
    #[allow(dead_code)]
    permit: Permit<'a>,
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.value.get() }
    }
}

pub struct WriteGuard<'a, T> {
    rwlock: &'a RwLock<T>,
    // WriteGuard owns the permit such that when it goes out of scope, the
    // permits will be added to the semaphore again
    #[allow(dead_code)]
    permit: Permit<'a>,
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.rwlock.value.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;
    extern crate std;
    use alloc::vec;
    use std::{sync::Arc, thread, time::Duration};

    #[test]
    fn basic_test() {
        let rwlock = Arc::new(RwLock::new(0x41));
        let mut handles = vec![];

        let mut write = rwlock.try_write().unwrap();
        *write = 0x42;
        drop(write);

        for i in 0..100 {
            let rw_lock_clone = rwlock.clone();
            let handle = thread::spawn(move || {
                let read = rw_lock_clone.read().unwrap();

                assert_eq!(*read, 0x42);
                thread::sleep(Duration::from_millis(1000));
            });
            handles.push(handle);
        }

        thread::sleep(Duration::from_millis(100));
        assert!(matches!(rwlock.try_write(), Err(())));

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        assert_eq!(rwlock.try_write().is_ok(), true);
    }
}
