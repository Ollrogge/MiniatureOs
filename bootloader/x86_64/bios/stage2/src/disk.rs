use crate::{dap, println};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    StartInSectors(u64),
    End(i64),
    Current(i64),
}

pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> u64;
}

#[repr(align(2))]
pub struct AlignedArrayBuffer<const LEN: usize> {
    pub buffer: [u8; LEN],
}

pub trait AlignedBuffer {
    fn slice(&self) -> &[u8];
    fn slice_mut(&mut self) -> &mut [u8];
}

impl<const LEN: usize> AlignedBuffer for AlignedArrayBuffer<LEN> {
    fn slice(&self) -> &[u8] {
        &self.buffer[..]
    }
    fn slice_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[..]
    }
}

pub trait Read {
    /// read exact amount of bytes and return it. Current disk position does not
    /// need to be sector aligned
    unsafe fn read_bytes(&mut self, len: usize) -> &[u8];
    /// Read complete sectors from disk into buf. Buf needs to be a multiple of
    /// sector size
    fn read_sectors(&mut self, sectors_amount: usize, buf: &mut [u8]);
    /// Read data into buffer. Buffer must be aligned to sector size
    fn read(&mut self, buf: &mut [u8]);
}

pub trait Disk {
    fn set_sector_size(&mut self, size: usize);
    fn sector_size(&self) -> usize;
    fn set_cluster_size(&mut self, size: usize);
    fn cluster_size(&self) -> usize;
    fn sectors_per_cluster(&self) -> usize;
}

#[derive(Clone)]
pub struct DiskAccess {
    pub disk_number: u16,
    // both offsets are byte offets. not LBA
    pub base_offset: u64,
    pub offset: u64,
    pub sector_size: usize,
    pub cluster_size: usize,
}

// TODO: dont harcode
// 512 bytes are enough to read the BPB and the properly set sector size and cluster size
pub const DEFAULT_SECTOR_SIZE: usize = 512;

impl DiskAccess {
    pub fn new(disk_number: u16, base_lba: u64, offset: u64) -> DiskAccess {
        DiskAccess {
            disk_number,
            base_offset: base_lba * DEFAULT_SECTOR_SIZE as u64,
            offset: offset * DEFAULT_SECTOR_SIZE as u64,
            sector_size: DEFAULT_SECTOR_SIZE,
            cluster_size: 0,
        }
    }

    pub fn set_sector_size(&mut self, size: usize) {
        self.sector_size = size;
    }
}

impl Disk for DiskAccess {
    fn set_sector_size(&mut self, size: usize) {
        self.sector_size = size
    }

    fn sector_size(&self) -> usize {
        self.sector_size
    }

    fn set_cluster_size(&mut self, size: usize) {
        self.cluster_size = size
    }

    fn cluster_size(&self) -> usize {
        self.cluster_size
    }

    fn sectors_per_cluster(&self) -> usize {
        self.cluster_size() / self.sector_size()
    }
}

impl Seek for DiskAccess {
    fn seek(&mut self, pos: SeekFrom) -> u64 {
        match pos {
            SeekFrom::Start(off) => self.offset = off,
            SeekFrom::StartInSectors(off) => self.offset = off * self.sector_size as u64,
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
    unsafe fn read_bytes(&mut self, len: usize) -> &[u8] {
        let current_sector_offset =
            usize::try_from(self.offset as usize % self.sector_size).unwrap();

        static mut TMP_BUF: AlignedArrayBuffer<1024> = AlignedArrayBuffer {
            buffer: [0; DEFAULT_SECTOR_SIZE * 2],
        };
        let buf = unsafe { &mut TMP_BUF };
        assert!(current_sector_offset + len <= buf.buffer.len());

        // read 2 sectors
        self.read(&mut buf.buffer);

        // only return a slice of length bytes at offset
        &buf.buffer[current_sector_offset..][..len]
    }

    fn read(&mut self, buf: &mut [u8]) {
        self.read_sectors(buf.len() / self.sector_size, buf)
    }

    fn read_sectors(&mut self, sectors_amount: usize, buf: &mut [u8]) {
        assert_eq!(buf.len() % self.sector_size, 0);
        assert!(buf.len() / self.sector_size >= sectors_amount);

        let mut start_lba = (self.base_offset + self.offset) / self.sector_size as u64;
        let end_addr = self.base_offset + self.offset + (sectors_amount * self.sector_size) as u64;

        let mut remaining_sector_count = sectors_amount as u64;
        let mut buffer_address = buf.as_ptr() as u32;

        while remaining_sector_count > 0 {
            let sector_count = u64::min(remaining_sector_count, 0x20) as u16;
            let packet = dap::DiskAddressPacket::new(buffer_address, sector_count, start_lba);

            unsafe {
                packet.load(self.disk_number);
            }

            remaining_sector_count -= u64::from(sector_count);
            start_lba += u64::from(sector_count);
            buffer_address += u32::from(sector_count) * self.sector_size as u32;
        }

        self.offset = end_addr;
    }
}
