//! Multi-producer, single-consumer (MPSC) queue
//!
//! Current implementation is a small version of
//! https://mycelium.elizas.website/cordyceps/struct.mpscqueue
//!

use crate::intrusive_linked_list::Linked;
use core::{
    cell::UnsafeCell,
    marker::PhantomPinned,
    ptr::{self, NonNull},
    sync::atomic::{
        AtomicBool, AtomicPtr,
        Ordering::{AcqRel, Acquire, Relaxed, Release},
    },
};

pub struct Links<T> {
    /// The next node in the queue.
    next: AtomicPtr<T>,

    /// Linked list links must always be `!Unpin`, in order to ensure that they
    /// never recieve LLVM `noalias` annotations; see also
    /// <https://github.com/rust-lang/rust/issues/63818>.
    _unpin: PhantomPinned,
}

impl<T> Links<T> {
    pub const fn new() -> Self {
        Self {
            next: AtomicPtr::new(ptr::null_mut()),
            _unpin: PhantomPinned,
        }
    }
}

impl<T> Default for Links<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MpscQueue<T: Linked<Links<T>>> {
    /// used by producer and consumer
    head: UnsafeCell<*mut T>,
    /// only used by consumer
    tail: AtomicPtr<T>,
    has_consumer: AtomicBool,

    stub: NonNull<T>,
    stub_is_static: bool,
}

unsafe impl<T> Send for MpscQueue<T>
where
    T: Send + Linked<Links<T>>,
    T::Handle: Send,
{
}
unsafe impl<T: Send + Linked<Links<T>>> Sync for MpscQueue<T> {}

/// Errors returned by [`MpscQueue::try_dequeue`] and [`Consumer::try_dequeue`].
#[derive(Debug, Eq, PartialEq)]
pub enum TryDequeueError {
    /// No element was dequeued because the queue was empty.
    Empty,

    Inconsistent,

    Busy,
}

pub struct Consumer<'q, T: Linked<Links<T>>> {
    q: &'q MpscQueue<T>,
}

impl<'q, T: Send + Linked<Links<T>>> Consumer<'q, T> {
    pub fn dequeue(&self) -> Option<T::Handle> {
        unsafe {
            // Safety: we can be sure we have exclusive consume access to queue
            self.q.dequeue_unchecked()
        }
    }

    pub fn try_dequeue(&self) -> Result<T::Handle, TryDequeueError> {
        unsafe {
            // Safety: we can be sure we have exclusive consume access to queue
            self.q.try_dequeue_unchecked()
        }
    }
}

impl<'q, T: Linked<Links<T>>> Consumer<'q, T> {
    pub fn new(q: &'q MpscQueue<T>) -> Self {
        Self { q }
    }
}

impl<T: Linked<Links<T>>> Drop for Consumer<'_, T> {
    fn drop(&mut self) {
        self.q.has_consumer.store(false, Relaxed);
    }
}

impl<T: Linked<Links<T>>> MpscQueue<T> {
    pub fn new() -> Self
    where
        T::Handle: Default,
    {
        Self::new_with_stub(Default::default())
    }

    pub fn new_with_stub(stub: T::Handle) -> Self {
        let stub = T::into_ptr(stub);
        let ptr = stub.as_ptr();

        Self {
            head: UnsafeCell::new(ptr),
            tail: AtomicPtr::new(ptr),
            has_consumer: AtomicBool::new(false),
            stub_is_static: false,
            stub,
        }
    }

    pub const unsafe fn new_with_static_stub(stub: &'static T) -> Self {
        let ptr = stub as *const T as *mut T;
        Self {
            head: UnsafeCell::new(ptr),
            tail: AtomicPtr::new(ptr),
            has_consumer: AtomicBool::new(false),
            stub_is_static: true,
            stub: NonNull::new_unchecked(ptr),
        }
    }

    pub fn enqueue(&self, element: T::Handle) {
        let ptr = T::into_ptr(element);

        self.enqueue_inner(ptr)
    }

    fn enqueue_inner(&self, ptr: NonNull<T>) {
        unsafe { Self::links(ptr).next.store(ptr::null_mut(), Relaxed) };
        let ptr = ptr.as_ptr();
        let prev = self.tail.swap(ptr, AcqRel);
        unsafe {
            Self::links(NonNull::new(prev).expect("Enqueue: prev == null"))
                .next
                .store(ptr, Release)
        }
    }

    pub fn dequeue(&self) -> Option<T::Handle> {
        loop {
            match self.try_dequeue() {
                Ok(val) => return Some(val),
                Err(TryDequeueError::Empty) => return None,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }

    pub fn try_dequeue(&self) -> Result<T::Handle, TryDequeueError> {
        if self
            .has_consumer
            .compare_exchange(false, true, AcqRel, Acquire)
            .is_err()
        {
            return Err(TryDequeueError::Busy);
        }

        // Safety: the `has_consumer` flag ensures mutual exclusion of
        // consumers.
        let res = unsafe { self.try_dequeue_unchecked() };

        self.has_consumer.store(false, Release);
        res
    }

    pub unsafe fn dequeue_unchecked(&self) -> Option<T::Handle> {
        loop {
            match self.try_dequeue_unchecked() {
                Ok(val) => return Some(val),
                Err(TryDequeueError::Empty) => return None,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }

    unsafe fn try_dequeue_unchecked(&self) -> Result<T::Handle, TryDequeueError> {
        // ptr to head pointer
        let head = self.head.get();
        let mut head_node = NonNull::new(*head).ok_or(TryDequeueError::Empty)?;

        let mut next = Self::links(head_node).next.load(Acquire);

        // Skip stub as it is not a real element
        if head_node == self.stub {
            let next_node = NonNull::new(next).ok_or(TryDequeueError::Empty)?;

            *head = next;
            head_node = next_node;
            next = Self::links(next_node).next.load(Acquire);
        }

        // there was at least 1 element in the queue, so return that
        if !next.is_null() {
            *head = next;
            return Ok(T::from_ptr(head_node));
        }

        let tail = self.tail.load(Acquire);
        if head_node.as_ptr() != tail {
            return Err(TryDequeueError::Inconsistent);
        }

        // re-enqueue stub to re-initialize queue
        self.enqueue_inner(self.stub);

        // There might have happenened concurrent enqueues, which would enable
        // use to return an element after all
        next = Self::links(head_node).next.load(Acquire);
        if next.is_null() {
            return Err(TryDequeueError::Empty);
        }

        *head = next;

        // head_node == next
        Ok(T::from_ptr(head_node))
    }

    unsafe fn links<'a>(ptr: NonNull<T>) -> &'a Links<T> {
        T::links(ptr).as_ref()
    }

    fn try_lock_consumer(&self) -> Option<()> {
        self.has_consumer
            .compare_exchange(false, true, AcqRel, Acquire)
            .map(|_| ())
            .ok()
    }

    pub fn try_consume(&self) -> Option<Consumer<'_, T>> {
        self.try_lock_consumer().map(|_| Consumer::new(self))
    }
}

impl<T: Linked<Links<T>>> Drop for MpscQueue<T> {
    fn drop(&mut self) {
        // Safety: because `Drop` is called with `&mut self`, we have
        // exclusive ownership over the queue, so it's always okay to touch
        // the tail cell.
        let mut current = unsafe { *self.head.get() };

        while let Some(node) = NonNull::new(current) {
            unsafe {
                let links = Self::links(node);
                let next = links.next.load(Relaxed);

                // Skip dropping the stub node; it is owned by the queue and
                // will be dropped when the queue is dropped. If we dropped it
                // here, that would cause a double free!
                if node != self.stub {
                    // Convert the pointer to the owning handle and drop it.
                    drop(T::from_ptr(node));
                }

                current = next;
            }
        }

        unsafe {
            // If the stub is static, don't drop it.
            if !self.stub_is_static {
                drop(T::from_ptr(self.stub));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use core::{fmt, pin::Pin};
    use std::{boxed::Box, println, sync::Arc, thread, vec::Vec};

    struct TestStruct {
        links: Links<TestStruct>,
        val: u64,
    }

    impl TestStruct {
        pub fn new(val: u64) -> Self {
            Self {
                links: Links::new(),
                val,
            }
        }
    }

    impl fmt::Debug for TestStruct {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestStruct")
                .field("val", &self.val)
                .finish()
        }
    }

    impl std::cmp::PartialEq for TestStruct {
        fn eq(&self, other: &Self) -> bool {
            self.val == other.val
        }
    }

    unsafe impl Linked<Links<TestStruct>> for TestStruct {
        type Handle = Pin<Box<Self>>;

        fn into_ptr(handle: Self::Handle) -> NonNull<TestStruct> {
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

    #[test]
    fn dequeue_empty() {
        let stub = Box::pin(TestStruct::new(0));
        let queue = MpscQueue::<TestStruct>::new_with_stub(stub);
        assert_eq!(queue.dequeue().is_none(), true)
    }

    #[test]
    fn dequeue_busy() {
        let stub = Box::pin(TestStruct::new(0));
        let queue = MpscQueue::<TestStruct>::new_with_stub(stub);

        let consumer = queue.try_consume().expect("Unable to acquire consumer ?");
        assert_eq!(consumer.try_dequeue(), Err(TryDequeueError::Empty));

        queue.enqueue(Box::pin(TestStruct::new(2)));

        assert_eq!(queue.try_dequeue(), Err(TryDequeueError::Busy));

        assert_eq!(consumer.try_dequeue(), Ok(Box::pin(TestStruct::new(2))),);

        assert_eq!(queue.try_dequeue(), Err(TryDequeueError::Busy));

        assert_eq!(consumer.try_dequeue(), Err(TryDequeueError::Empty));

        queue.enqueue(Box::pin(TestStruct::new(2)));
        drop(consumer);
        assert_eq!(queue.try_dequeue(), Ok(Box::pin(TestStruct::new(2))));
        assert_eq!(queue.try_dequeue(), Err(TryDequeueError::Empty));
    }

    #[test]
    fn basic_test() {
        const THREADS: u64 = 10;
        const MSGS: u64 = 100;
        let stub = Box::pin(TestStruct::new(1));
        let queue = Arc::new(MpscQueue::<TestStruct>::new_with_stub(stub));

        let threads: Vec<_> = (0..THREADS)
            .map(|thread| {
                let q = queue.clone();
                thread::spawn(move || {
                    for i in 0..MSGS {
                        q.enqueue(Box::pin(TestStruct::new(i)));
                        println!("thread {thread}; msg {i}{MSGS}");
                    }
                })
            })
            .collect();

        let mut i = 0;
        while i < THREADS * MSGS {
            match queue.try_dequeue() {
                Ok(msg) => {
                    i += 1;
                    println!("recv {} ({i}/{})", msg.val, THREADS * MSGS);
                }
                Err(TryDequeueError::Busy) => {
                    panic!("the queue should never be busy, as there is only one consumer")
                }
                Err(e) => {
                    println!("recv error {e:?}");
                    thread::yield_now();
                }
            }
        }

        for thread in threads {
            thread.join().unwrap();
        }
    }
}
