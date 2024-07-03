#![no_std]
pub mod intrusive_linked_list;
pub mod mutex;

#[macro_export]
macro_rules! const_assert {
    ($($tt:tt)*) => {
        const _: () = assert!($($tt)*);
    }
}
