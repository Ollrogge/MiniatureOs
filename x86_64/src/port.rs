use core::{arch::asm, marker::PhantomData};
trait WritePort {
    unsafe fn write_to_register(port: u16, val: Self);
}

trait ReadPort {
    unsafe fn read_from_register(port: u16) -> Self;
}

impl ReadPort for u8 {
    unsafe fn read_from_register(port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al")value, in("dx")port,
                 options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl ReadPort for u16 {
    unsafe fn read_from_register(port: u16) -> u16 {
        let value: u16;
        unsafe {
            asm!("in ax, dx", out("ax")value, in("dx")port,
                 options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl ReadPort for u32 {
    unsafe fn read_from_register(port: u16) -> u32 {
        let value: u32;
        unsafe {
            asm!("in eax, dx", out("eax")value, in("dx")port,
                 options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl WritePort for u8 {
    unsafe fn write_to_register(port: u16, val: u8) {
        unsafe {
            asm!("out dx, al", in("dx")port, in("al")val,
            options(nomem, nostack, preserves_flags));
        }
    }
}

impl WritePort for u16 {
    unsafe fn write_to_register(port: u16, value: u16) {
        unsafe {
            asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags));
        }
    }
}

impl WritePort for u32 {
    unsafe fn write_to_register(port: u16, val: u32) {
        unsafe {
            asm!("out dx, eax", in("dx")port, in("eax")val,
            options(nomem, nostack, preserves_flags));
        }
    }
}

pub struct Port<T> {
    address: u16,
    phantom: PhantomData<T>,
}

impl<T> Port<T> {
    pub const fn new(address: u16) -> Self {
        Port {
            address,
            phantom: PhantomData,
        }
    }
}

impl<T: ReadPort> Port<T> {
    pub fn read(&self) -> T {
        unsafe { T::read_from_register(self.address) }
    }
}

impl<T: WritePort> Port<T> {
    pub fn write(&self, val: T) {
        unsafe { T::write_to_register(self.address, val) }
    }
}

impl<T> PartialEq for Port<T> {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

// write to an used port to cause a small delay (1-4 microseconds)
// necessary on older machines to give PIC some time to react to commands as they
// might now be preserved fast enough
pub fn io_wait() {
    unsafe {
        asm!("out dx, al", in("dx") 0x80u16, in("al") 0u8, options(nomem, nostack, preserves_flags));
    }
}
