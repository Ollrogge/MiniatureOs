#![no_std]
#![no_main]
use api::FramebufferInfo;
use core::{arch::asm, mem::size_of};
use x86_64::memory::{MemoryRegion, PhysicalMemoryRegion, PhysicalMemoryRegionType};

pub mod bump_frame_allocator;
pub mod mbr;
pub mod realmode;

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

#[derive(Debug, Default)]
#[repr(C)]
pub struct BiosInfo {
    pub stage4: PhysicalMemoryRegion,
    pub kernel: PhysicalMemoryRegion,
    pub framebuffer: FramebufferInfo,
    pub last_physical_address: u64,
    // cant pass a pointer here since it will be corrupted when switching
    // from protected to long mode because pointer size differs
    pub memory_map_address: u64,
    pub memory_map_size: u64,
}

impl BiosInfo {
    pub fn new(
        stage4: PhysicalMemoryRegion,
        kernel: PhysicalMemoryRegion,
        framebuffer: FramebufferInfo,
        last_physical_address: u64,
        // cant use arr because I dont know how many mem regions there are
        memory_map_address: u64,
        memory_map_size: u64,
    ) -> BiosInfo {
        Self {
            stage4,
            kernel,
            framebuffer,
            last_physical_address,
            memory_map_address,
            memory_map_size,
        }
    }
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
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

impl Into<PhysicalMemoryRegionType> for E820MemoryRegionType {
    fn into(self) -> PhysicalMemoryRegionType {
        match self {
            E820MemoryRegionType::Normal => PhysicalMemoryRegionType::Free,
            _ => PhysicalMemoryRegionType::Reserved,
        }
    }
}

/// Memory information returned by BIOS 0xe820 command
#[derive(Default, Clone, Copy, Debug)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub typ: E820MemoryRegionType,
    pub acpi_extended_attributes: u32,
}
const_assert!(size_of::<E820MemoryRegion>() == 24);

impl E820MemoryRegion {
    pub const fn empty() -> Self {
        Self {
            start: 0,
            size: 0,
            typ: E820MemoryRegionType::Unusable,
            acpi_extended_attributes: 0,
        }
    }
}

impl Into<PhysicalMemoryRegion> for E820MemoryRegion {
    fn into(self) -> PhysicalMemoryRegion {
        PhysicalMemoryRegion::new(self.start, self.size, self.typ.into())
    }
}

impl Into<PhysicalMemoryRegion> for &E820MemoryRegion {
    fn into(self) -> PhysicalMemoryRegion {
        PhysicalMemoryRegion::new(self.start, self.size, self.typ.into())
    }
}

impl MemoryRegion for E820MemoryRegion {
    fn start(&self) -> u64 {
        self.start
    }

    fn set_start(&mut self, start: u64) {
        self.start = start
    }

    fn end(&self) -> u64 {
        self.start + self.size
    }

    fn size(&self) -> u64 {
        self.size
    }

    fn set_size(&mut self, size: u64) {
        self.size = size
    }

    fn contains(&self, address: u64) -> bool {
        self.start() <= address && address <= self.end()
    }

    fn is_usable(&self) -> bool {
        self.typ == E820MemoryRegionType::Normal
    }
}
