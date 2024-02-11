//! Memory information
//! https://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map (c code)
use core::{arch::asm, convert::AsRef, mem::size_of};

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

#[derive(Default)]
pub struct MemoryMap {
    map: [E820MemoryRegion; 0x20],
    size: usize,
}

impl MemoryMap {
    pub fn get() -> Result<MemoryMap, ()> {
        let mut obj = Self::default();
        const MAGIC_NUMBER: u32 = 0x534D4150;
        let mut offset = 0x0;
        let mut signature = MAGIC_NUMBER;
        let mut len = 0x0;
        let mut entries = 0x0;

        loop {
            unsafe {
                asm!(
                    "int 0x15",
                    inout("eax") 0xE820 => signature,
                    inout("ecx") size_of::<E820MemoryRegion>() => len,
                    inout("ebx") offset,
                    in("edx") MAGIC_NUMBER,
                    in("edi") &obj.map[offset],
                    options(nostack)
                );
            }

            if signature != MAGIC_NUMBER {
                return Err(());
            }

            let entry = &obj.map[entries];

            if len > 0x20 && (entry.acpi_extended_attributes & 0x1) == 0 {
                continue;
            }

            entries += 1;

            if offset == 0 || entries > size_of::<MemoryMap>() {
                break;
            }
        }

        obj.size = entries;

        Ok(obj)
    }

    pub fn iter(&self) -> impl Iterator<Item = &E820MemoryRegion> {
        self.map[..self.size].iter()
    }
}
