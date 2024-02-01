#![no_std]
#![no_main]
use core::arch::asm;
use core::panic::PanicInfo;

pub mod dap;
pub mod disk;
pub mod fat;
pub mod gdt;
pub mod mbr;
pub mod memory_map;
pub mod print;
pub mod vesa;

#[macro_export]
macro_rules! const_assert {
    ($($tt:tt)*) => {
        const _: () = assert!($($tt)*);
    }
}

pub fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        hlt();
    }
}

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    panic!("Fail called with code: {:x}", code);
}

pub struct Region {
    start: u64,
    len: u64,
}

#[repr(C)]
pub struct BiosInfo {
    kernel: Region,
}
