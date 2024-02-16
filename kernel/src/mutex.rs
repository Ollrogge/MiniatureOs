use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    pub inner: UnsafeCell<T>,
    pub lock_status: AtomicBool,
}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Self {
        Self {
            inner: UnsafeCell::new(val),
            lock_status: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        loop {
            core::hint::spin_loop();
            match self.lock_status.compare_exchange(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        MutexGuard::new(self)
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> MutexGuard<'a, T> {
    pub fn new(mutex: &'a Mutex<T>) -> Self {
        Self { mutex }
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.inner.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.inner.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.lock_status.store(false, Ordering::Release);
    }
}
