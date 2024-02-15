#![no_std]
#![no_main]
use core::arch::asm;
use x86_64::memory::MemoryRegion;

pub mod gdt;
pub mod mbr;
pub mod mutex;
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
    pub start: u64,
    pub len: u64,
}

impl Region {
    pub fn new(start: u64, len: u64) -> Region {
        Region { start, len }
    }
}

#[repr(C)]
pub struct BiosInfo<'a> {
    pub stage4: Region,
    pub kernel: Region,
    pub framebuffer: BiosFramebufferInfo,
    pub last_physical_address: u64,
    pub memory_map: &'a [E820MemoryRegion],
}

impl<'a> BiosInfo<'a> {
    pub fn new(
        stage4: Region,
        kernel: Region,
        framebuffer: BiosFramebufferInfo,
        last_physical_address: u64,
        memory_map: &'a [E820MemoryRegion],
    ) -> BiosInfo {
        Self {
            stage4,
            kernel,
            framebuffer,
            last_physical_address,
            memory_map,
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

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Debug)]
#[repr(u32)]
pub enum E820MemoryRegionType {
    #[default]
    None,
    Normal,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    Unusable,
}

/// Memory information returned by BIOS 0xe820 command
#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start: u64,
    pub length: u64,
    pub typ: E820MemoryRegionType,
    pub acpi_extended_attributes: u32,
}

impl MemoryRegion for E820MemoryRegion {
    fn start(&self) -> u64 {
        self.start
    }

    fn end(&self) -> u64 {
        self.start + self.length
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn contains(&self, address: u64) -> bool {
        self.start() <= address && address <= self.end()
    }
}
