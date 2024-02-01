use core::{arch::asm, cell::UnsafeCell, mem::size_of};

pub struct RacyCell<T>(UnsafeCell<T>);

impl<T> RacyCell<T> {
    pub const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }

    /// Gets a mutable pointer to the wrapped value.
    ///
    /// ## Safety
    /// Ensure that the access is unique (no active references, mutable or not).
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

unsafe impl<T> Send for RacyCell<T> where T: Send {}
unsafe impl<T: Sync> Sync for RacyCell<T> {}

static MEMORY_MAP: RacyCell<[E820MemoryRegion; 0x20]> = RacyCell::new(
    [E820MemoryRegion {
        start: 0,
        length: 0,
        typ: E820MemoryRegionType::None,
        acpi_extended_attributes: 0,
    }; 0x20],
);

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

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct E820MemoryRegion {
    pub start: u64,
    pub length: u64,
    pub typ: E820MemoryRegionType,
    pub acpi_extended_attributes: u32,
}

// https://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map (c code)
pub fn load_memory_map() -> Result<&'static [E820MemoryRegion], ()> {
    const MAGIC_NUMBER: u32 = 0x534D4150;

    let mut offset = 0x0;
    let mut signature = MAGIC_NUMBER;
    let memory_map = unsafe { MEMORY_MAP.get_mut() };
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
                in("edi") &memory_map[offset],
                options(nostack, nomem)
            );
        }

        if signature != MAGIC_NUMBER {
            return Err(());
        }

        let entry = memory_map[entries];

        if len > 0x20 && (entry.acpi_extended_attributes & 0x1) == 0 {
            continue;
        }

        entries += 1;

        if offset == 0 || entries > size_of::<[E820MemoryRegion; 0x20]>() {
            break;
        }
    }

    Ok(&mut memory_map[..entries])
}
