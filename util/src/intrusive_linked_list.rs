//! Implementation of an intrusive doubly linked list.
//! An intrusive linked list is a type of linked list where the data elements
//! themselves contain the pointers to the next.
//!
//! In usual linked lists the list contains a data pointer to the data
//!
//! The current linked list implementation is basically small version of
//! https://mycelium.elizas.website/cordyceps/list/index.html
//!
use core::{
    cell::UnsafeCell,
    marker::PhantomPinned,
    mem,
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr,
    ptr::NonNull,
};

pub struct BoxAt<T: ?Sized> {
    ptr: *mut T,
}

impl<T> BoxAt<T> {
    pub fn new(address: usize, value: T) -> Self {
        let ptr = address as *mut T;
        unsafe {
            ptr::write(ptr, value);
        }

        Self { ptr }
    }

    pub unsafe fn from_raw(ptr: *mut T) -> BoxAt<T> {
        Self { ptr }
    }

    pub fn leak(self) -> &'static mut T {
        unsafe { &mut *self.ptr }
    }

    pub fn pin(address: usize, value: T) -> Pin<Self> {
        unsafe { Pin::new_unchecked(Self::new(address, value)) }
    }
}

impl<T> AsMut<T> for BoxAt<T> {
    fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> AsRef<T> for BoxAt<T> {
    fn as_ref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> Deref for BoxAt<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> DerefMut for BoxAt<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

type Link<T> = Option<NonNull<T>>;

pub unsafe trait Linked<L> {
    type Handle;
    fn into_ptr(r: Self::Handle) -> NonNull<Self>;
    unsafe fn from_ptr(ptr: NonNull<Self>) -> Self::Handle;
    unsafe fn links(ptr: NonNull<Self>) -> NonNull<L>;
}

pub struct Links<T: ?Sized> {
    inner: UnsafeCell<LinksInner<T>>,
}

unsafe impl<T: Send> Send for Links<T> {}
unsafe impl<T: Sync> Sync for Links<T> {}

#[repr(C)]
struct LinksInner<T: ?Sized> {
    next: Link<T>,
    prev: Link<T>,
    /// Linked list links must always be `!Unpin`, in order to ensure that they
    /// never recieve LLVM `noalias` annotations; see also
    /// <https://github.com/rust-lang/rust/issues/63818>.
    _unpin: PhantomPinned,
}

/// Intrusive doubly linked list.
pub struct IntrusiveLinkedList<T: Linked<Links<T>> + ?Sized> {
    head: Link<T>,
    tail: Link<T>,
    len: usize,
}

unsafe impl<T: Linked<Links<T>> + ?Sized> Send for IntrusiveLinkedList<T> where T: Send {}
unsafe impl<T: Linked<Links<T>> + ?Sized> Sync for IntrusiveLinkedList<T> where T: Sync {}

impl<T: Linked<Links<T>> + ?Sized> IntrusiveLinkedList<T> {
    /// Creates a new, empty IntrusiveLinkedList.
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    pub fn pop_back(&mut self) -> Option<T::Handle> {
        let tail = self.tail?;
        self.len -= 1;

        unsafe {
            let mut tail_links = T::links(tail);
            self.tail = tail_links.as_ref().prev();
            debug_assert_eq!(
                tail_links.as_ref().next(),
                None,
                "the tail node must not have a next link"
            );

            if let Some(prev) = tail_links.as_mut().prev() {
                T::links(prev).as_mut().set_next(None);
            } else {
                self.head = None;
            }

            tail_links.as_mut().unlink();
            Some(T::from_ptr(tail))
        }
    }

    pub fn pop_front(&mut self) -> Option<T::Handle> {
        let head = self.head?;
        self.len -= 1;

        unsafe {
            let mut head_links = T::links(head);
            self.head = head_links.as_ref().next();
            if let Some(next) = head_links.as_mut().next() {
                T::links(next).as_mut().set_prev(None);
            } else {
                self.tail = None;
            }

            head_links.as_mut().unlink();
            Some(T::from_ptr(head))
        }
    }

    pub fn push_back(&mut self, item: T::Handle) {
        let ptr = T::into_ptr(item);
        assert_ne!(self.tail, Some(ptr));
        unsafe {
            T::links(ptr).as_mut().set_next(None);
            T::links(ptr).as_mut().set_prev(self.tail);
            if let Some(tail) = self.tail {
                T::links(tail).as_mut().set_next(Some(ptr));
            }
        }

        self.tail = Some(ptr);
        if self.head.is_none() {
            self.head = Some(ptr);
        }

        self.len += 1;
    }

    pub fn push_front(&mut self, item: T::Handle) {
        let ptr = T::into_ptr(item);
        assert_ne!(self.head, Some(ptr));
        unsafe {
            T::links(ptr).as_mut().set_next(self.head);
            T::links(ptr).as_mut().set_prev(None);
            if let Some(head) = self.head {
                T::links(head).as_mut().set_prev(Some(ptr));
            }
        }

        self.head = Some(ptr);

        if self.tail.is_none() {
            self.tail = Some(ptr);
        }

        self.len += 1;
    }

    pub fn front(&self) -> Option<Pin<&T>> {
        let head = self.head?;
        let pin = unsafe { Pin::new_unchecked(head.as_ref()) };
        Some(pin)
    }

    pub fn front_mut(&self) -> Option<Pin<&mut T>> {
        let mut head = self.head?;
        let pin = unsafe { Pin::new_unchecked(head.as_mut()) };
        Some(pin)
    }

    pub fn back(&self) -> Option<Pin<&T>> {
        let tail = self.tail?;
        let pin = unsafe { Pin::new_unchecked(tail.as_ref()) };
        Some(pin)
    }

    pub fn back_mut(&self) -> Option<Pin<&mut T>> {
        let mut tail = self.tail?;
        let pin = unsafe { Pin::new_unchecked(tail.as_mut()) };
        Some(pin)
    }

    /// Returns `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// Returns the length of the list.
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T: ?Sized> Links<T> {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(LinksInner {
                next: None,
                prev: None,
                _unpin: PhantomPinned,
            }),
        }
    }

    pub fn is_linked(&self) -> bool {
        self.next().is_some() || self.prev().is_some()
    }

    fn unlink(&mut self) {
        self.inner.get_mut().next = None;
        self.inner.get_mut().prev = None;
    }

    fn next(&self) -> Link<T> {
        unsafe { (*self.inner.get()).next }
    }

    fn prev(&self) -> Link<T> {
        unsafe { (*self.inner.get()).prev }
    }

    fn set_next(&mut self, next: Link<T>) -> Link<T> {
        mem::replace(&mut self.inner.get_mut().next, next)
    }

    fn set_prev(&mut self, prev: Link<T>) -> Link<T> {
        mem::replace(&mut self.inner.get_mut().prev, prev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::boxed::Box;
    struct TestStruct {
        links: Links<TestStruct>,
        val: u64,
    }

    impl TestStruct {
        pub fn new(val: u64) -> Self {
            Self {
                val,
                links: Links::new(),
            }
        }
    }

    unsafe impl Linked<Links<TestStruct>> for TestStruct {
        type Handle = Pin<Box<Self>>;

        fn into_ptr(handle: Pin<Box<TestStruct>>) -> NonNull<TestStruct> {
            unsafe { NonNull::from(Box::leak(Pin::into_inner_unchecked(handle))) }
        }

        unsafe fn from_ptr(ptr: NonNull<TestStruct>) -> Pin<Box<TestStruct>> {
            Pin::new_unchecked(Box::from_raw(ptr.as_ptr()))
        }

        unsafe fn links(target: NonNull<TestStruct>) -> NonNull<Links<TestStruct>> {
            let links = ptr::addr_of_mut!((*target.as_ptr()).links);

            NonNull::new_unchecked(links)
        }
    }

    #[test]
    fn test_list() {
        let mut list = IntrusiveLinkedList::<TestStruct>::new();

        for i in 0..5 {
            list.push_back(Box::pin(TestStruct::new(i)));
            assert_eq!(list.len(), (i + 1) as usize);
        }

        for i in 0..5 {
            let e = list.pop_front().unwrap();

            assert_eq!(e.val, i);
        }

        assert!(list.pop_front().is_none());
        assert!(list.pop_back().is_none());

        list.push_back(Box::pin(TestStruct::new(1)));

        assert_eq!(list.front().unwrap().val, list.back().unwrap().val);

        list.push_front(Box::pin(TestStruct::new(2)));

        assert_eq!(list.pop_back().unwrap().val, 1);
        assert_eq!(list.pop_front().unwrap().val, 2);
    }
}
