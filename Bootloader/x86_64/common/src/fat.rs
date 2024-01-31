use crate::println;

use crate::disk::{Read, Seek, SeekFrom};
// BIOS Parameter block
#[derive(Debug)]
pub struct Bpb {
    bytes_per_sector: u16,
    // clust = smallest unit of space allocation for files and dirs on FAT fs
    sectors_per_cluster: u8,
    // number of sectors before first FAT
    reserved_sector_count: u16,
    // amount of copies of FAT present on disk. Multiple used due to redundancy
    // reasons. If one FAT is damaged it can be repaired using the backuped one
    fat_count: u8,
    // number of root directory entries
    root_entry_count: u16,
    // total number of sectors on disk
    total_sectors_16: u16,
    total_sectors_32: u32,
    // Size of the FAT data structure in sectors
    fat16_size: u16,
    fat32_size: u32,
    // start cluster of root directory
    root_cluster: u32,
}

#[derive(PartialEq)]
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

// https://wiki.osdev.org/FAT
impl Bpb {
    pub fn parse<D: Read + Seek>(disk: &mut D) -> Self {
        disk.seek(SeekFrom::Start(0));
        let mut raw = [0u8; 512];
        disk.read_exact(&mut raw);

        let bytes_per_sector = u16::from_le_bytes(raw[11..13].try_into().unwrap());
        let sectors_per_cluster = raw[13];
        let reserved_sector_count = u16::from_le_bytes(raw[14..16].try_into().unwrap());
        let fat_count = raw[16];
        let root_entry_count = u16::from_le_bytes(raw[17..19].try_into().unwrap());
        let fat16_size = u16::from_le_bytes(raw[22..24].try_into().unwrap());

        let total_sectors_16 = u16::from_le_bytes(raw[19..21].try_into().unwrap());
        let total_sectors_32 = u32::from_le_bytes(raw[32..36].try_into().unwrap());

        let root_cluster;
        let fat32_size;

        // FAT12 or FAT16
        if total_sectors_16 != 0 && total_sectors_32 == 0 {
            fat32_size = 0;
            root_cluster = 0;
        }
        // FAT32
        else if total_sectors_32 != 0 && total_sectors_16 == 0 {
            fat32_size = u32::from_le_bytes(raw[36..40].try_into().unwrap());
            root_cluster = u32::from_le_bytes(raw[44..48].try_into().unwrap());
        } else {
            panic!("ExactlyOneTotalSectorsFieldMustBeZero");
        }

        Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sector_count,
            fat_count,
            root_entry_count,
            total_sectors_16,
            total_sectors_32,
            fat16_size,
            fat32_size,
            root_cluster,
        }
    }

    fn root_dir_sectors(&self) -> u32 {
        ((self.root_entry_count as u32 * 32) + (self.bytes_per_sector as u32 - 1))
            / self.bytes_per_sector as u32
    }

    fn count_of_clusters(&self) -> u32 {
        let total_sectors = if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u32
        } else {
            self.total_sectors_32
        };
        let data_sectors = total_sectors
            - (self.reserved_sector_count as u32
                + (self.fat_count as u32 * self.fat_size())
                + self.root_dir_sectors());
        data_sectors / self.sectors_per_cluster as u32
    }

    fn fat_size(&self) -> u32 {
        if self.fat16_size != 0 && self.fat32_size == 0 {
            self.fat16_size as u32
        } else {
            debug_assert!(self.fat16_size == 0 && self.fat32_size != 0);
            self.fat32_size
        }
    }

    fn fat_type(&self) -> FatType {
        let count_of_clusters = self.count_of_clusters();
        if count_of_clusters < 4085 {
            FatType::Fat12
        } else if count_of_clusters < 65525 {
            FatType::Fat16
        } else {
            FatType::Fat32
        }
    }

    fn first_data_sector(&self) -> u32 {
        self.reserved_sector_count as u32
            + (self.fat_count as u32 * self.fat_size())
            + self.root_dir_sectors()
    }

    fn first_root_dir_sector(&self) -> u32 {
        self.first_data_sector() - self.root_dir_sectors()
    }

    fn root_dir_size(&self) -> u16 {
        let size = self.root_entry_count * 32;

        // 16384
        assert!(size == 512 * 32);

        size
    }
}

enum FileAttributes {
    ReadOnly = 0x1,
    Hidden = 0x2,
    System = 0x4,
    VolumeId = 0x8,
    Directory = 0x10,
    Archive = 0x20,
}

enum DirEntry {
    NormalDirEntry,
    LongNameDirEntry,
}

// inlcudes only fields needed for loading next stages
struct NormalDirEntry {
    filename: [char; 11],
    attributes: u8,
    first_cluster: u32,
    size: u32,
}

struct LongNameDirEntry {
    order: u8,
}

pub struct FileSystem<D> {
    disk: D,
    bpb: Bpb,
}

impl<D: Read + Seek> FileSystem<D> {
    pub fn parse(mut disk: D) -> Self {
        Self {
            disk,
            bpb: Bpb::parse(&mut disk),
        }
    }

    fn read_root_dir(&mut self, buffer: &[u8]) {
        if self.bpb.fat_type() == FatType::Fat32 {
            unimplemented!();
        }

        assert!(buffer.len() == self.bpb.root_dir_size() as usize);

        self.disk
            .seek(SeekFrom::Start((self.bpb.first_root_dir_sector())));

        let first_sector = self.bpb.first_root_dir_sector();
    }
}
