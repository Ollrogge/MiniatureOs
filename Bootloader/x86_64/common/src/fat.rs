use crate::{print, println};
use core::{char::DecodeUtf16Error, convert::TryFrom, slice, str};

fn copy_bytes(dst: &mut [u8], src: & [u8]) {
    assert!(src.len() <= dst.len());
    let min_len = src.len().min(dst.len());
    for i in 0..min_len {
        dst[i] = src[i];
    }
}

use crate::disk::{Read, Seek, SeekFrom};
// BIOS Parameter block
#[derive(Debug)]
pub struct Bpb {
    bytes_per_sector: u16,
    // cluster = smallest unit of space allocation for files and dirs on FAT fs
    sectors_per_cluster: u8,
    // number of sectors before first FAT
    reserved_sector_count: u16,
    // amount of copies of FAT present on disk. Multiple used due to redundancy
    // reasons. If one FAT is damaged it can be repaired using the backup'ed one
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

type Error = ();

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

#[derive(PartialEq, Default, Clone)]
#[repr(u8)]
enum FileAttribute {
    #[default]
    None,
    ReadOnly = 0x1,
    Hidden = 0x2,
    System = 0x4,
    VolumeId = 0x8,
    Directory = 0x10,
    Archive = 0x20,
    LongFileName = FileAttribute::ReadOnly as u8
        | FileAttribute::Hidden as u8
        | FileAttribute::System as u8
        | FileAttribute::VolumeId as u8,
}

impl TryFrom<u8> for FileAttribute {
    type Error = ();
    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            x if x == FileAttribute::ReadOnly as u8 => Ok(FileAttribute::ReadOnly),
            x if x == FileAttribute::Hidden as u8 => Ok(FileAttribute::Hidden),
            x if x == FileAttribute::System as u8 => Ok(FileAttribute::System),
            x if x == FileAttribute::VolumeId as u8 => Ok(FileAttribute::VolumeId),
            x if x == FileAttribute::Directory as u8 => Ok(FileAttribute::Directory),
            x if x == FileAttribute::Archive as u8 => Ok(FileAttribute::Archive),
            x if x == FileAttribute::LongFileName as u8 => Ok(FileAttribute::LongFileName),
            _ => Err(()),
        }
    }
}

pub enum DirEntry {
    Unused,
    EndOfDir,
    NormalDirEntry(NormalDirEntry),
    LongNameDirEntry(LongNameDirEntry),
}

impl DirEntry {
    fn parse(raw: &[u8]) -> Result<DirEntry, Error> {
        println!("Attributes: {:#x}", raw[11]);
        let attributes: FileAttribute = raw[11].try_into()?;

        if attributes == FileAttribute::LongFileName {
            let mut entry = LongNameDirEntry::default();

            entry.order = raw[0];
            copy_bytes(&raw[])
            entry.name1 = &raw[1..11];
            entry.name2 = &raw[14..26];
            entry.name3 = &raw[28..32];

            Ok(DirEntry::LongNameDirEntry(entry))
        } else {
            let mut entry = NormalDirEntry::default();
            copy_bytes(&mut entry.filename, &raw[0..11]);
            entry.attributes = attributes;
            entry.first_cluster = u32::from(u16::from_le_bytes(raw[20..22].try_into().unwrap()))
                << 16
                | u32::from(u16::from_le_bytes(raw[26..28].try_into().unwrap()));

            entry.size = u32::from_le_bytes(raw[28..32].try_into().unwrap());

            Ok(DirEntry::NormalDirEntry(entry))
        }
    }
}

// inlcudes only fields needed for loading next stages
#[derive(Default)]
pub struct NormalDirEntry {
    pub filename: [u8; 11],
    attributes: FileAttribute,
    first_cluster: u32,
    size: u32,
}

// only when VFAT, completely stores the max 255 bytes long filename as a chain
// the NormalDirEntry does **not** store the filename when VFAT is supported
// only a fallback in cases VFAT isn't
#[derive(Default)]
pub struct LongNameDirEntry {
    pub order: u8,
    name: [u8; 255],
}

impl<'a> LongNameDirEntry {
    pub fn name(&self) -> impl Iterator<Item = Result<char, DecodeUtf16Error>> + 'a {
        let iter = self
            .name1
            .chunks(2)
            .chain(self.name2.chunks(2))
            .chain(self.name3.chunks(2))
            .map(|c| u16::from_le_bytes(c.try_into().unwrap()))
            .take_while(|&c| c != 0);
        char::decode_utf16(iter)
    }

    pub fn print_name(&self) {
        for c in self.name().filter_map(|e| e.ok()) {
            print!("{}", c)
        }
    }
}

struct File {
    start_sector: u32,
    cluster_count: u32,
}

pub struct FileSystem<D> {
    disk: D,
    bpb: Bpb,
}

impl<D: Read + Seek> FileSystem<D> {
    pub fn parse(mut disk: D) -> Self {
        Self {
            bpb: Bpb::parse(&mut disk),
            disk,
        }
    }

    pub fn read_root_dir<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> impl Iterator<Item = Result<DirEntry, ()>> {
        assert!(buffer.len() == self.bpb.root_dir_size() as usize);

        if self.bpb.fat_type() == FatType::Fat32 {
            unimplemented!();
        }

        self.disk
            .seek(SeekFrom::Start(u64::from(self.bpb.first_root_dir_sector())));

        self.disk.read_exact(buffer);

        buffer
            .chunks(0x20)
            .take_while(|e| e[0] != Self::END_OF_DIRECTORY)
            .filter(|e| e[0] != Self::UNUSED_ENTRY)
            .map(DirEntry::parse)
    }

    pub fn find_file_in_root_dir(name: &str) -> Option<File> {}
}

struct RootDirIter<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> RootDirIter<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        RootDirIter { buf, offset: 0}
    }

    const END_OF_DIRECTORY: u8 = 0x0;
    const UNUSED_ENTRY: u8 = 0xe5;
    pub fn next_entry(&mut self) -> Result<DirEntry, Error> {
        match self.buf[self.offset] {
            Self::END_OF_DIRECTORY => Ok(DirEntry::EndOfDir),
            Self::UNUSED_ENTRY => Ok(DirEntry::Unused),
            _ => {
                let attributes: FileAttribute = self.buf[self.offset + 11].try_into()?;

                Error
            }
        }
    }
}

impl<'a> Iterator for RootDirIter<'a> {
    type Item = Result<DirEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_entry() {
            Ok(entry) => {
                match entry {
                    DirEntry::EndOfDir => None,
                    DirEntry::Unused => self.next(),
                    _ => Some(Ok(entry))
                }
            },
            Err(e) => Some(Err(e))
        }
    }
}