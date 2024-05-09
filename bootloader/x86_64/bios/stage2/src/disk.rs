use crate::{dap, println};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
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
    fn read_sectors(&mut self, buf: &mut [u8]);
}

pub trait Disk {
    fn set_sector_size(&mut self, size: usize);
    fn get_sector_size(&self) -> usize;
    fn set_cluster_size(&mut self, size: usize);
    fn get_cluster_size(&self) -> usize;
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

    fn get_sector_size(&self) -> usize {
        self.sector_size
    }

    fn set_cluster_size(&mut self, size: usize) {
        self.cluster_size = size
    }

    fn get_cluster_size(&self) -> usize {
        self.cluster_size
    }

    fn sectors_per_cluster(&self) -> usize {
        self.get_cluster_size() / self.get_sector_size()
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
    unsafe fn read_bytes(&mut self, len: usize) -> &[u8] {
        let current_sector_offset =
            usize::try_from(self.offset as usize % DEFAULT_SECTOR_SIZE).unwrap();

        static mut TMP_BUF: AlignedArrayBuffer<1024> = AlignedArrayBuffer {
            buffer: [0; DEFAULT_SECTOR_SIZE * 2],
        };
        let buf = unsafe { &mut TMP_BUF };
        assert!(current_sector_offset + len <= buf.buffer.len());

        // read 2 sectors
        self.read_sectors(&mut buf.buffer);

        // only return a slice of length bytes at offset
        &buf.buffer[current_sector_offset..][..len]
    }

    fn read_sectors(&mut self, buf: &mut [u8]) {
        assert_eq!(buf.len() % self.sector_size, 0);

        let end_addr = self.base_offset + self.offset + u64::try_from(buf.len()).unwrap();
        let mut start_lba = (self.base_offset + self.offset) / self.sector_size as u64;
        let end_lba = (end_addr - 1) / self.sector_size as u64;

        let mut sector_count = end_lba + 1 - start_lba;
        let mut buffer_addr = buf.as_ptr_range().start as u32;

        while sector_count > 0 {
            let sectors_read_cnt = u64::min(sector_count, 0x20) as u16;
            let packet = dap::DiskAddressPacket::new(buffer_addr, sectors_read_cnt, start_lba);

            unsafe {
                packet.load(self.disk_number);
            }

            sector_count -= u64::from(sectors_read_cnt);
            start_lba += u64::from(sectors_read_cnt);
            buffer_addr += u32::from(sectors_read_cnt) * self.sector_size as u32;
        }

        self.offset = end_addr;
    }

    /*
    fn read_exact(&mut self, buf: &mut [u8]) {
        // TODO: read it based on SECTOR_SIZE stored in BPB ?
        // TODO: make offset a byte offset instead of sector to enable bytewise access ?
        let mut start_lba = self.base_lba + self.offset;
        let mut sector_count =
            ((buf.len() as u64 + (self.sector_size as u64 - 1)) / self.sector_size as u64) as u32;
        let mut buffer_address = buf.as_ptr() as u32;

        self.offset += u64::from(sector_count);

        while sector_count > 0 {
            let sectors = u32::min(sector_count, 0x20) as u16;
            let packet = dap::DiskAddressPacket::new(buffer_address, sectors, start_lba);

            unsafe {
                packet.load(self.disk_number);
            }

            sector_count -= u32::from(sectors);
            start_lba += u64::from(sectors);
            buffer_address += u32::from(sectors) * self.sector_size as u32;
        }
    }
    */
}
