#![no_std]
#![no_main]
use core::{arch::asm, arch::global_asm, panic::PanicInfo};

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    print_char(code);
    loop {
        hlt();
    }
}

pub fn print(s: &str) {
    for c in s.chars() {
        if c.is_ascii() {
            print_char(c as u8);
        } else {
            print_char(b'.');
        }
    }
}

/// Write Teletype to Active Page
fn print_char(c: u8) {
    unsafe {
        asm!("mov ah, 0x0E; xor bh, bh; int 0x10", in("al") c);
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

#[repr(C, packed)]
pub struct DiskAddressPacket {
    size: u8,
    zero: u8,
    sector_count: u16,
    segment: u16,
    offset: u16,
    start_lba: u64,
}

impl DiskAddressPacket {
    pub fn new(buffer_address: u32, sector_count: u16, start_lba: u64) -> DiskAddressPacket {
        DiskAddressPacket {
            size: 0x10,
            zero: 0,
            sector_count,
            segment: (buffer_address & 0xffff) as u16,
            offset: (buffer_address >> 16) as u16,
            start_lba: start_lba.to_le(),
        }
    }

    // https://wiki.osdev.org/BIOS
    // https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
    pub unsafe fn load(&self, disk_number: u8) {
        let self_addr = self as *const Self as u16;
        unsafe {
            asm!("push 'h'", "mov si, {0:x}", "int 0x13", "jc fail", "pop si", in(reg) self_addr, in("ah") 0x42u8, in("dl") disk_number);
        };
    }
}
