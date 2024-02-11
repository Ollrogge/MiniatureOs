use core::{arch::asm, panic::PanicInfo};
pub(crate) trait UnwrapOrFail {
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

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn hlt() {
    unsafe {
        asm!("hlt");
    }
}

/// Write Teletype to Active Page
pub fn print_char(c: u8) {
    unsafe {
        asm!("mov ah, 0x0E; xor bh, bh; int 0x10", in("al") c);
    }
}

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    print_char(code);
    loop {
        hlt();
    }
}

// local def because print in util is too big (overflows mbr code sec)
pub(crate) fn print(s: &[u8]) {
    for &c in s.iter() {
        print_char(c);
    }
}
