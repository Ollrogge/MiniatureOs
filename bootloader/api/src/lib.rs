#![no_std]
use core::ops::{Deref, DerefMut};
use x86_64::memory::{MemoryRegion, PhysicalMemoryRegion};

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct FramebufferInfo {
    pub region: PhysicalMemoryRegion,
    pub width: u16,
    pub height: u16,
    pub bytes_per_pixel: u8,
    pub stride: u16,
    pub pixel_format: PixelFormat,
}

impl FramebufferInfo {
    pub fn new(
        region: PhysicalMemoryRegion,
        width: u16,
        height: u16,
        bytes_per_pixel: u8,
        stride: u16,
        pixel_format: PixelFormat,
    ) -> FramebufferInfo {
        FramebufferInfo {
            region,
            width,
            height,
            bytes_per_pixel,
            stride,
            pixel_format,
        }
    }
}

pub struct PhysicalMemoryRegions {
    ptr: *mut PhysicalMemoryRegion,
    len: usize,
}

impl PhysicalMemoryRegions {
    pub fn new(ptr: *mut PhysicalMemoryRegion, len: usize) -> Self {
        Self { ptr, len }
    }
}

impl Deref for PhysicalMemoryRegions {
    type Target = [PhysicalMemoryRegion];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for PhysicalMemoryRegions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

pub struct BootInfo {
    pub kernel: PhysicalMemoryRegion,
    pub framebuffer: FramebufferInfo,
    pub memory_regions: PhysicalMemoryRegions,
}

impl BootInfo {
    pub fn new(
        kernel: PhysicalMemoryRegion,
        framebuffer: FramebufferInfo,
        memory_regions: PhysicalMemoryRegions,
    ) -> Self {
        Self {
            kernel,
            framebuffer,
            memory_regions,
        }
    }
}
