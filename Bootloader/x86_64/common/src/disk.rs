use crate::dap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> u64;
}

pub trait Read {
    fn read_exact(&mut self, buf: &mut [u8]);
}

pub struct DiskAccess {
    pub disk_number: u8,
    // both have sectors as unit
    pub base_lba: u64,
    pub offset: u64,
}

const SECTOR_SIZE: u64 = 512;

impl DiskAccess {
    pub fn new(disk_number: u8, base_lba: u64, offset: u64) -> DiskAccess {
        DiskAccess {
            disk_number,
            base_lba,
            offset,
        }
    }
}

impl Seek for DiskAccess {
    fn seek(&mut self, pos: SeekFrom) -> u64 {
        match pos {
            SeekFrom::Start(off) => self.offset = off,
            SeekFrom::Current(off) => {
                self.offset = if off > 0 {
                    self.offset.saturating_add(off as u64)
                } else {
                    self.offset.saturating_sub((-off) as u64)
                }
            }
            _ => unimplemented!(),
        }

        self.offset
    }
}

impl Read for DiskAccess {
    fn read_exact(&mut self, buf: &mut [u8]) {
        // todo: read it based on SECTOR_SIZE stored in BPB ?
        let mut start_lba = self.base_lba + self.offset;
        let mut sector_count = ((buf.len() as u64 + (SECTOR_SIZE - 1)) / SECTOR_SIZE) as u32;
        let mut buffer_address = buf.as_ptr() as u32;

        while sector_count > 0 {
            let sectors = u32::min(sector_count, 0x20) as u16;
            let packet = dap::DiskAddressPacket::new(buffer_address, sectors, start_lba);

            unsafe {
                packet.load(self.disk_number);
            }

            sector_count -= u32::from(sectors);
            start_lba += u64::from(sectors);
            buffer_address += u32::from(sectors) * SECTOR_SIZE as u32;
        }
    }
}
