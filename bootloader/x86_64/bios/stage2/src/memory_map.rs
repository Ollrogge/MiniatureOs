//! Memory information
//! https://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map (c code)
use common::E820MemoryRegion;
use core::{arch::asm, convert::AsRef, mem::size_of};

#[derive(Default)]
pub struct MemoryMap {
    pub map: [E820MemoryRegion; 0x20],
    pub size: usize,
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
