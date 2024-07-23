#![no_std]
pub mod hashmap;
pub mod intrusive_linked_list;
pub mod mutex;
#[cfg(feature = "kernel")]
pub mod range_allocator;
pub mod volatile;

#[macro_export]
macro_rules! const_assert {
    ($($tt:tt)*) => {
        const _: () = assert!($($tt)*);
    }
}
