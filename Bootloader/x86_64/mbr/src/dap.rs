use crate::util::UnwrapOrFail;
use core::arch::asm;

#[repr(C, packed)]
pub struct DiskAddressPacket {
    size: u8,
    zero: u8,
    sector_count: u16,
    offset: u16,
    segment: u16,
    start_lba: u64,
}

impl DiskAddressPacket {
    // real mode memory addressing: PhysicalAddress = segment * 16 + offset
    // so: offset = last 4 bits, segment = address >> 4
    pub fn new(buffer_address: u32, sector_count: u16, start_lba: u64) -> Self {
        Self {
            size: 0x10,
            zero: 0,
            sector_count,
            offset: (buffer_address & 0b1111) as u16,
            segment: (buffer_address >> 4).try_into().unwrap_or_fail(b'o'),
            start_lba: start_lba.to_le(),
        }
    }

    // https://wiki.osdev.org/BIOS
    // https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
    pub unsafe fn load(&self, disk_number: u8) {
        let self_addr = self as *const Self as u16;
        unsafe {
            asm!("push 'h'", "mov si, {0:x}", "int 0x13", "jc fail", "pop si", in(reg) self_addr, in("ah") 0x42u8, in("dl") disk_number);
        };
    }
}
