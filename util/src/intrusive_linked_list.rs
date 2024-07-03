//! Implementation of an intrusive doubly linked list.
//! An intrusive linked list is a type of linked list where the data elements
//! themselves contain the pointers to the next.
//!
//! In usual linked lists the list contains a data pointer to the data
//!
use core::{mem::offset_of, ptr::NonNull};

macro_rules! container_of {
    ($ptr:expr, $type:path, $member:ident) => {
        $ptr.cast::<u8>()
            .add(offset_of!($type, $member))
            .cast::<$type>()
    };
}

struct ListNode {
    next: Option<NonNull<ListNode>>,
    prev: Option<NonNull<ListNode>>,
}

impl ListNode {
    pub fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }

    pub fn get_next(&self) -> Option<NonNull<ListNode>> {
        self.next
    }

    pub fn set_next(&mut self, next: Option<NonNull<ListNode>>) {
        self.next = next;
    }

    pub fn get_prev(&self) -> Option<NonNull<ListNode>> {
        self.prev
    }

    pub fn set_prev(&mut self, prev: Option<NonNull<ListNode>>) {
        self.prev = prev;
    }
}

struct IntrusiveLinkedList {
    head: Option<NonNull<ListNode>>,
    tail: Option<NonNull<ListNode>>,
    len: usize,
}

impl IntrusiveLinkedList {
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    pub fn push_front(&mut self, new: &mut ListNode) {
        let mut new = unsafe { NonNull::new_unchecked(new as *mut ListNode) };
        match self.head {
            Some(mut head) => {
                unsafe {
                    new.as_mut().set_next(Some(head));
                    head.as_mut().set_prev(Some(new));
                }

                self.head = Some(new);
            }
            None => {
                self.head = Some(new);
                self.tail = Some(new);
            }
        }

        self.len += 1;
    }

    pub fn push_back(&mut self, new: &mut ListNode) {
        let mut new = unsafe { NonNull::new_unchecked(new as *mut ListNode) };
        match self.tail {
            Some(mut tail) => {
                unsafe {
                    tail.as_mut().set_next(Some(new));
                    new.as_mut().set_prev(Some(tail));
                }
                self.tail = Some(new);
            }
            None => {
                self.head = Some(new);
                self.tail = Some(new);
            }
        }

        self.len += 1;
    }

    pub fn pop_front(&mut self) -> Option<NonNull<ListNode>> {
        match self.head {
            Some(head) => {
                let tail = self.tail.unwrap();
                if head == tail {
                    self.head = None;
                    self.tail = None;
                } else {
                    self.head = unsafe { head.as_ref().get_next() };
                    self.head
                        .map(|mut head| unsafe { head.as_mut().set_next(None) });
                }

                self.len -= 1;

                Some(head)
            }
            None => None,
        }
    }

    pub fn pop_back(&mut self) -> Option<NonNull<ListNode>> {
        match self.tail {
            Some(tail) => {
                let head = self.head.unwrap();
                if head == tail {
                    self.head = None;
                    self.tail = None;
                } else {
                    self.tail = unsafe { tail.as_ref().get_prev() };
                    self.tail
                        .map(|mut tail| unsafe { tail.as_mut().set_next(None) });
                }

                self.len -= 1;

                Some(tail)
            }
            None => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

struct TestStruct {
    next: ListNode,
    val: u64,
}

impl TestStruct {
    pub fn new(val: u64) -> Self {
        Self {
            next: ListNode::new(),
            val,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list() {
        let mut list = IntrusiveLinkedList::new();

        let mut t1 = TestStruct::new(1);
        let mut t2 = TestStruct::new(2);
        let mut t3 = TestStruct::new(3);
        let mut t4 = TestStruct::new(4);

        list.push_back(&mut t1.next);
        assert!(list.len() == 1);
        list.push_back(&mut t2.next);
        assert!(list.len() == 2);
        list.push_front(&mut t3.next);
        assert!(list.len() == 3);
        list.push_front(&mut t4.next);
        assert!(list.len() == 4);

        let t4_2 = unsafe { &*container_of!(list.pop_front().unwrap().as_ptr(), TestStruct, next) };
        assert!(t4_2.val == t4.val);

        let t2_2 = unsafe { &*container_of!(list.pop_back().unwrap().as_ptr(), TestStruct, next) };
        assert!(t2_2.val == t2.val);

        let t1_2 = unsafe { &*container_of!(list.pop_back().unwrap().as_ptr(), TestStruct, next) };
        assert!(t1_2.val == t1.val);

        // head == front now
        let t3_2 = unsafe { &*container_of!(list.pop_back().unwrap().as_ptr(), TestStruct, next) };
        assert!(t3_2.val == t3.val);

        assert!(list.pop_front().is_none());
        assert!(list.pop_back().is_none());
    }
}
