//! This module is responsible for detecting available memory using x86 BIOS
//! functions
use common::E820MemoryRegion;
use core::{arch::asm, convert::AsRef, mem::size_of};
use x86_64::mutex::{Mutex, MutexGuard};

pub static MEMORY_MAP: Mutex<MemoryMap> = Mutex::new(MemoryMap {
    map: [E820MemoryRegion::empty(); 0x20],
    size: 0,
});

#[derive(Default)]
pub struct MemoryMap {
    pub map: [E820MemoryRegion; 0x20],
    pub size: usize,
}

impl MemoryMap {
    /// Detecting memory using BIOS function 0xe820
    /// https://wiki.osdev.org/Detecting_Memory_(x86)#BIOS_Function:_INT_0x15.2C_EAX_.3D_0xE820
    pub fn get() -> Result<MutexGuard<'static, MemoryMap>, ()> {
        const MAGIC_NUMBER: u32 = 0x534D4150;
        const QUERY_SYTEM_ADDRESS_MAP_CMD: u32 = 0xE820;
        let mut cont_id = 0x0;
        let mut signature = 0x0;
        let mut len = 0x0;
        let mut entries_cnt = 0x0;

        let mut memory_map = MEMORY_MAP.lock();

        loop {
            unsafe {
                asm!(
                    "int 0x15",
                    inout("eax") QUERY_SYTEM_ADDRESS_MAP_CMD => signature,
                    inout("ecx") size_of::<E820MemoryRegion>() => len,
                    inout("ebx") cont_id,
                    in("edx") MAGIC_NUMBER,
                    in("edi") &memory_map.map[entries_cnt],
                    options(nostack)
                );
            }

            if signature != MAGIC_NUMBER {
                return Err(());
            }

            let entry = &mut memory_map.map[entries_cnt];

            if len > 0x20 && (entry.acpi_extended_attributes & 0x1) == 0 {
                continue;
            }

            entries_cnt += 1;

            if cont_id == 0 || entries_cnt > size_of::<MemoryMap>() {
                break;
            }
        }

        memory_map.size = entries_cnt;

        Ok(memory_map)
    }

    pub fn iter(&self) -> impl Iterator<Item = &E820MemoryRegion> {
        self.map[..self.size].iter()
    }
}
