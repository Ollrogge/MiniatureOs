/*
use core::ptr::NonNull;
pub struct Node<I> {
    next: Option<NonNull<Node<I>>>,
    val: I,
}
pub struct LinkedList<I> {
    head: Option<NonNull<Node<I>>>,
}

impl<I> LinkedList<I> {
    pub fn new() -> Self {
        Self { head: None }
    }

    pub fn insert(&mut self, val: I) {
        if self.head.is_none() {
            let node = Node { next: None, val };
            self.head = unsafe { Some(NonNull::new_unchecked(node)) };
        }
    }
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
