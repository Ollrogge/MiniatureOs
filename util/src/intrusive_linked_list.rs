//! Implementation of an intrusive doubly linked list.
//! An intrusive linked list is a type of linked list where the data elements
//! themselves contain the pointers to the next.
//!
//! In usual linked lists the list contains a data pointer to the data
//!
use core::{mem::offset_of, ptr::NonNull};

/// Macro to obtain the container structure from a pointer to one of its members.
macro_rules! container_of {
    ($ptr:expr, $type:path, $member:ident) => {
        $ptr.cast::<u8>()
            .add(offset_of!($type, $member))
            .cast::<$type>()
    };
}

/// Node of the intrusive linked list.
pub struct ListNode {
    next: Option<NonNull<ListNode>>,
    prev: Option<NonNull<ListNode>>,
}

impl ListNode {
    /// Creates a new ListNode.
    pub fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }

    /// Returns the next node, if any.
    pub fn next(&self) -> Option<&ListNode> {
        self.next.as_ref().map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns the next node as mutable, if any.
    pub fn next_mut(&mut self) -> Option<&mut ListNode> {
        self.next.as_mut().map(|mut ptr| unsafe { ptr.as_mut() })
    }

    /// Sets the next node.
    pub fn set_next(&mut self, next: Option<NonNull<ListNode>>) {
        self.next = next;
    }

    /// Returns the previous node, if any.
    pub fn prev(&self) -> Option<&ListNode> {
        self.prev.as_ref().map(|ptr| unsafe { ptr.as_ref() })
    }

    /// Returns the previous node as mutable, if any.
    pub fn prev_mut(&mut self) -> Option<&mut ListNode> {
        self.prev.as_mut().map(|mut ptr| unsafe { ptr.as_mut() })
    }

    /// Sets the previous node.
    pub fn set_prev(&mut self, prev: Option<NonNull<ListNode>>) {
        self.prev = prev;
    }

    /// Returns the address of the ListNode.
    pub fn address(&self) -> usize {
        self as *const ListNode as usize
    }

    pub fn as_ptr(&mut self) -> *mut ListNode {
        self as *mut ListNode
    }

    /// Creates a new ListNode at the given address.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it allows creating a reference to an
    /// arbitrary memory location.
    pub unsafe fn new_at_address(address: usize) -> &'static mut ListNode {
        let node = &mut *(address as *mut ListNode);
        node.next = None;
        node.prev = None;
        node
    }
}

/// Intrusive doubly linked list.
pub struct IntrusiveLinkedList {
    head: Option<NonNull<ListNode>>,
    tail: Option<NonNull<ListNode>>,
    len: usize,
}

impl IntrusiveLinkedList {
    /// Creates a new, empty IntrusiveLinkedList.
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Adds a node to the front of the list.
    pub fn push_front(&mut self, new: &mut ListNode) {
        let mut new = NonNull::new(new as *mut ListNode).unwrap();
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

    /// Adds a node to the back of the list.
    pub fn push_back(&mut self, new: &mut ListNode) {
        let mut new = NonNull::new(new as *mut ListNode).unwrap();
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

    /// Removes and returns the node from the front of the list.
    pub fn pop_front(&mut self) -> Option<&mut ListNode> {
        self.head.map(|mut head| {
            if self.head == self.tail {
                self.head = None;
                self.tail = None;
            } else {
                self.head = unsafe { head.as_ref().next().map(|n| NonNull::from(n)) };
                if let Some(mut new_head) = self.head {
                    unsafe { new_head.as_mut().set_prev(None) };
                }
            }
            self.len -= 1;
            unsafe { head.as_mut() }
        })
    }

    /// Removes and returns the node from the back of the list.
    pub fn pop_back(&mut self) -> Option<&mut ListNode> {
        self.tail.map(|mut tail| {
            if self.head == self.tail {
                self.head = None;
                self.tail = None;
            } else {
                self.tail = unsafe { tail.as_ref().prev().map(|p| NonNull::from(p)) };
                if let Some(mut new_tail) = self.tail {
                    unsafe { new_tail.as_mut().set_next(None) };
                }
            }
            self.len -= 1;
            unsafe { tail.as_mut() }
        })
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

#[cfg(test)]
mod tests {
    use super::*;
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
