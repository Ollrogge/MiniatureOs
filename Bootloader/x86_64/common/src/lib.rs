#![no_std]
#![no_main]
use core::arch::asm;

pub mod dap;
pub mod disk;
pub mod fat;
pub mod gdt;
pub mod mbr;
pub mod memory_map;
pub mod uart;

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

#[no_mangle]
pub extern "C" fn fail(code: u8) -> ! {
    panic!("Fail called with code: {:x}", code);
}

#[derive(Clone, Copy)]
pub struct Region {
    start: u64,
    len: u64,
}

impl Region {
    pub fn new(start: u64, len: u64) -> Region {
        Region { start, len }
    }
}

#[repr(C)]
pub struct BiosInfo {
    kernel: Region,
    framebuffer: BiosFramebufferInfo,
}

impl BiosInfo {
    pub fn new(kernel: Region, framebuffer: BiosFramebufferInfo) -> BiosInfo {
        Self {
            kernel,
            framebuffer,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct BiosFramebufferInfo {
    pub region: Region,
    pub width: u16,
    pub height: u16,
    pub bytes_per_pixel: u8,
    pub stride: u16,
    pub pixel_format: PixelFormat,
}

impl BiosFramebufferInfo {
    pub fn new(
        region: Region,
        width: u16,
        height: u16,
        bytes_per_pixel: u8,
        stride: u16,
        pixel_format: PixelFormat,
    ) -> BiosFramebufferInfo {
        BiosFramebufferInfo {
            region,
            width,
            height,
            bytes_per_pixel,
            stride,
            pixel_format,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub enum PixelFormat {
    Rgb,
    Bgr,
    Unknown {
        red_position: u8,
        green_position: u8,
        blue_position: u8,
    },
}
