#![feature(iterator_try_collect)]
#![no_std]

pub mod elf_loader;
#[cfg(feature = "kernel")]
pub mod hashmap;
pub mod intrusive_linked_list;
pub mod mpsc_queue;
pub mod mutex;
#[cfg(feature = "kernel")]
pub mod range_allocator;
pub mod ringbuffer;
pub mod rwlock;
mod semaphore;
pub mod volatile;

#[macro_export]
macro_rules! const_assert {
    ($($tt:tt)*) => {
        const _: () = assert!($($tt)*);
    }
}

/// Macro to obtain the container structure from a pointer to one of its members.
#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $type:path, $member:ident) => {
        $ptr.cast::<u8>()
            .add(offset_of!($type, $member))
            .cast::<$type>()
    };
}
