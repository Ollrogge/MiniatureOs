#![no_std]
#![no_main]
use core::arch::asm;

pub mod dap;
pub mod disk;
pub mod fat;
pub mod gdt;
pub mod mbr;
pub mod print;

pub fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    println!("Failed with code: {:x}", code);
    loop {
        hlt();
    }
}

pub trait UnwrapOrFail {
    type Out;

    fn unwrap_or_fail(self, code: u8) -> Self::Out;
}

impl<T> UnwrapOrFail for Option<T> {
    type Out = T;

    fn unwrap_or_fail(self, code: u8) -> Self::Out {
        match self {
            Some(v) => v,
            None => fail(code),
        }
    }
}

impl<T, E> UnwrapOrFail for Result<T, E> {
    type Out = T;

    fn unwrap_or_fail(self, code: u8) -> Self::Out {
        match self {
            Ok(v) => v,
            Err(_) => fail(code),
        }
    }
}
