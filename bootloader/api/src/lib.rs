#![no_std]
use core::ops::{Deref, DerefMut};
use x86_64::memory::{MemoryRegion, PhysicalMemoryRegion, VirtualMemoryRegion};

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub enum PixelFormat {
    #[default]
    Rgb,
    Bgr,
    Unknown {
        red_position: u8,
        green_position: u8,
        blue_position: u8,
    },
}

// This struct MUST NOT contain any usize types since it is passed between different
// CPU operating modes and therefore usize representation changes.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
#[repr(align(8))]
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

// This structure can contain usize since it is only passed on long mode
pub struct BootInfo {
    pub kernel: PhysicalMemoryRegion,
    pub kernel_stack: VirtualMemoryRegion,
    pub framebuffer: FramebufferInfo,
    pub memory_regions: PhysicalMemoryRegions,
    pub physical_memory_offset: usize,
}

impl BootInfo {
    pub fn new(
        kernel: PhysicalMemoryRegion,
        kernel_stack: VirtualMemoryRegion,
        framebuffer: FramebufferInfo,
        memory_regions: PhysicalMemoryRegions,
        physical_memory_offset: usize,
    ) -> Self {
        Self {
            kernel,
            kernel_stack,
            framebuffer,
            memory_regions,
            physical_memory_offset,
        }
    }
}
