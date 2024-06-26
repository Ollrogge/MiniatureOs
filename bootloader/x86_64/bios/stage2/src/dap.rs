//! This module implements disk access using BIOS function 0x42
//! https://wiki.osdev.org/BIOS
//! https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
use crate::println;
use core::arch::asm;

/// BIOS disk address packet
#[repr(C, packed)]
pub struct DiskAddressPacket {
    /// size of packet (16)
    size: u8,
    zero: u8,
    /// number of sectors to transfer
    sector_count: u16,
    /// 16 bit offset of transfer buffer address
    offset: u16,
    /// 16 bit segment of buffer address
    segment: u16,
    /// starting logical block address (lba)
    /// block = basically unique idenfitier for a sector
    /// LBA tells "where" on the disk (i.e., the sector's position).
    start_lba: u64,
}

impl DiskAddressPacket {
    pub fn new(buffer_address: u32, sector_count: u16, start_lba: u64) -> Self {
        Self {
            size: 0x10,
            zero: 0,
            sector_count,
            // real mode memory addressing:
            //  PhysicalAddress = segment * 16 + offset
            //  so: offset = last 4 bits, segment = address >> 4
            offset: (buffer_address & 0b1111) as u16,
            segment: (buffer_address >> 4)
                .try_into()
                .expect("Buffer address too big"),
            start_lba: start_lba.to_le(),
        }
    }

    /// Read data from disk using BIOS function 13
    /// https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
    pub unsafe fn load(&self, disk_number: u16) {
        let self_addr = self as *const Self as u16;
        unsafe {
            asm!(
                "push 'h'",
                "mov {1:x}, si",
                "mov si, {0:x}",
                "int 0x13",
                "jc fail",
                "pop si",
                "mov si, {1:x}",
                in(reg) self_addr,
                out(reg) _,
                in("ah") 0x42u8,
                in("dx") disk_number);
        };
    }
}
