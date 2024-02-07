use crate::{
    disk::{self, SECTOR_SIZE},
    print, println,
};
use core::{
    arch::asm, char::DecodeUtf16Error, convert::TryFrom, default::Default, ptr, slice, str,
};

// FAT is a very simple file system -- nothing more than a singly-linked list of clusters in a gigantic table

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

impl FatType {
    pub fn classification_threshold(&self) -> u32 {
        match self {
            FatType::Fat12 => 0xFF7,
            FatType::Fat16 => 0xFFF7,
            FatType::Fat32 => 0xFFFFFF7,
        }
    }
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

    fn first_fat_sector(&self) -> u32 {
        self.reserved_sector_count as u32
    }
}

#[derive(PartialEq, Default, Clone)]
struct FileAttributes(u8);

impl FileAttributes {
    const NONE: u8 = 0;
    const READ_ONLY: u8 = 0x1;
    const HIDDEN: u8 = 0x2;
    const SYSTEM: u8 = 0x4;
    const VOLUME_ID: u8 = 0x8;
    const DIRECTORY: u8 = 0x10;
    const ARCHIVE: u8 = 0x20;
    const LONG_FILE_NAME: u8 = Self::READ_ONLY | Self::HIDDEN | Self::SYSTEM | Self::VOLUME_ID;

    fn new() -> Self {
        FileAttributes(Self::NONE)
    }

    fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    fn unset(&mut self, flag: u8) {
        self.0 &= !flag;
    }

    fn is_set(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }
}

impl PartialEq<u8> for FileAttributes {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

impl PartialEq<FileAttributes> for u8 {
    fn eq(&self, other: &FileAttributes) -> bool {
        *self == other.0
    }
}

pub enum DirEntry {
    Unused,
    EndOfDir,
    NormalDirEntry(NormalDirEntry),
    LongNameDirEntry(LongNameDirEntry),
}

impl DirEntry {
    const END_OF_DIRECTORY: u8 = 0x0;
    const UNUSED_ENTRY: u8 = 0xe5;
    const NORMAL_ENTRY_SIZE: usize = 0x20;
    fn parse(raw: &[u8]) -> Result<(usize, DirEntry), Error> {
        if raw[0] == Self::END_OF_DIRECTORY {
            return Ok((Self::NORMAL_ENTRY_SIZE, DirEntry::EndOfDir));
        } else if raw[0] == Self::UNUSED_ENTRY {
            return Ok((Self::NORMAL_ENTRY_SIZE, DirEntry::Unused));
        }

        let attributes = FileAttributes(raw[11]);

        if attributes == FileAttributes::LONG_FILE_NAME {
            let mut long_name_entry = LongNameDirEntry::default();
            let mut name_idx = 0x0;
            let mut total_size = 0x0;

            for (i, entry) in raw.chunks(0x20).enumerate() {
                let attributes = FileAttributes(entry[11]);
                if attributes == FileAttributes::LONG_FILE_NAME {
                    let name1 = &entry[1..11];
                    let name2 = &entry[14..26];
                    let name3 = &entry[28..32];

                    let iter = name1
                        .chunks(2)
                        .chain(name2.chunks(2))
                        .chain(name3.chunks(2))
                        .map(|c| u16::from_le_bytes(c.try_into().unwrap()))
                        .take_while(|&c| c != 0);

                    for c in char::decode_utf16(iter).filter_map(|c| c.ok()) {
                        long_name_entry.filename[name_idx] = c;
                        name_idx += 1;
                    }
                // Long file name entries always have a regular 8.3 entry to
                // which they belong. The long file name entries are always
                // placed immediately before their 8.3 entry.
                } else {
                    long_name_entry.first_cluster =
                        u32::from(u16::from_le_bytes(entry[20..22].try_into().unwrap())) << 16
                            | u32::from(u16::from_le_bytes(entry[26..28].try_into().unwrap()));

                    long_name_entry.size = u32::from_le_bytes(entry[28..32].try_into().unwrap());

                    long_name_entry.attributes = attributes;

                    total_size = (i + 1) * Self::NORMAL_ENTRY_SIZE;
                    break;
                }
            }

            assert!(total_size != 0);

            Ok((total_size, DirEntry::LongNameDirEntry(long_name_entry)))
        } else {
            let mut entry = NormalDirEntry::default();
            for (i, &b) in raw[0..entry.filename.len()].iter().enumerate() {
                entry.filename[i] = char::from(b);
            }
            entry.attributes = attributes;
            entry.first_cluster = u32::from(u16::from_le_bytes(raw[20..22].try_into().unwrap()))
                << 16
                | u32::from(u16::from_le_bytes(raw[26..28].try_into().unwrap()));

            entry.size = u32::from_le_bytes(raw[28..32].try_into().unwrap());

            Ok((Self::NORMAL_ENTRY_SIZE, DirEntry::NormalDirEntry(entry)))
        }
    }

    pub fn eq_name(&self, name: &str) -> bool {
        match self {
            DirEntry::NormalDirEntry(e) => e
                .filename
                .iter()
                .cloned()
                .take_while(|&c| c != '\0')
                .eq(name.chars()),
            DirEntry::LongNameDirEntry(e) => e
                .filename
                .iter()
                .cloned()
                .take_while(|&c| c != '\0')
                .eq(name.chars()),
            _ => false,
        }
    }

    pub fn is_dir(&self) -> bool {
        match self {
            DirEntry::NormalDirEntry(e) => e.attributes.is_set(FileAttributes::DIRECTORY),
            DirEntry::LongNameDirEntry(e) => e.attributes.is_set(FileAttributes::DIRECTORY),
            _ => false,
        }
    }

    pub fn first_cluster(&self) -> u32 {
        match self {
            DirEntry::NormalDirEntry(e) => e.first_cluster,
            DirEntry::LongNameDirEntry(e) => e.first_cluster,
            _ => 0,
        }
    }

    pub fn file_size(&self) -> u32 {
        match self {
            DirEntry::NormalDirEntry(e) => e.size,
            DirEntry::LongNameDirEntry(e) => e.size,
            _ => 0,
        }
    }
}

// inlcudes only fields needed for loading next stages
#[derive(Default)]
pub struct NormalDirEntry {
    filename: [char; 11],
    attributes: FileAttributes,
    pub first_cluster: u32,
    // in bytes
    size: u32,
}

impl NormalDirEntry {
    pub fn print_filename(&self) {
        for c in self.filename.iter() {
            print!("{}", c);
        }
        print!("\r\n");
    }
}

// only when VFAT, completely stores the max 255 bytes long filename as a chain
// the NormalDirEntry does **not** store the filename when VFAT is supported
// only a fallback in cases VFAT isn't
pub struct LongNameDirEntry {
    order: u8,
    pub filename: [char; 255],
    pub first_cluster: u32,
    attributes: FileAttributes,
    size: u32,
}

impl LongNameDirEntry {
    pub fn print_filename(&self) {
        for c in self.filename.iter() {
            print!("{}", c);
        }
        print!("\r\n");
    }
}

impl Default for LongNameDirEntry {
    fn default() -> LongNameDirEntry {
        LongNameDirEntry {
            order: 0,
            filename: [0 as char; 255],
            attributes: FileAttributes::new(),
            first_cluster: 0,
            size: 0,
        }
    }
}

pub struct File {
    pub start_sector: u32,
    pub size: u32,
}

impl File {
    pub fn new(start_sector: u32, size: u32) -> File {
        File { start_sector, size }
    }
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
    ) -> impl Iterator<Item = Result<DirEntry, ()>> + 'a {
        assert!(buffer.len() == self.bpb.root_dir_size() as usize);

        if self.bpb.fat_type() == FatType::Fat32 {
            unimplemented!();
        }

        self.disk
            .seek(SeekFrom::Start(u64::from(self.bpb.first_root_dir_sector())));

        self.disk.read_exact(buffer);

        RootDirIter::new(buffer)
    }

    // The clusters of a file need not be right next to each other on the disk.
    // In fact it is likely that they are scattered widely throughout the disk
    // The FAT allows the operating system to follow the "chain" of clusters in a file.

    pub fn file_clusters<'a>(
        &'a mut self,
        file: &File,
    ) -> impl Iterator<Item = Result<Cluster, ()>> + 'a {
    }

    pub fn find_file_in_root_dir(&mut self, name: &str) -> Option<File> {
        // todo: somehow not hardcode this ?
        // FAT16: common to have a root directory with max 512 entries of size 32
        // If I had dynamic memory i could use bpb.root_entry_count
        let mut buffer = [0u8; 512 * 32];
        let mut entries = self.read_root_dir(&mut buffer).filter_map(|e| e.ok());
        let entry = entries.find(|e| e.eq_name(name))?;

        if entry.is_dir() {
            None
        } else {
            Some(File::new(entry.first_cluster(), entry.file_size()))
        }
    }

    pub fn try_load_file(&mut self, name: &str, dest: *mut u8) -> Result<(), Error> {
        let file = self.find_file_in_root_dir(name).ok_or(())?;
        let mut buffer = [0u8; SECTOR_SIZE];
        let mut sectors = file.size as usize / SECTOR_SIZE + 1;
        self.disk
            .seek(SeekFrom::Start(u64::from(file.start_sector)));

        for i in 0..sectors {
            self.disk.read_exact(&mut buffer);
            let dest = dest.wrapping_add(i * SECTOR_SIZE);

            //println!("Buf: {:?}", buffer);

            unsafe {
                ptr::copy_nonoverlapping(buffer.as_ptr(), dest, buffer.len());
            }
        }

        Ok(())
    }

    pub fn disk(&mut self) -> &mut D {
        &mut self.disk
    }
}

struct RootDirIter<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> RootDirIter<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        RootDirIter { buf, offset: 0 }
    }

    pub fn next_entry(&mut self) -> Result<DirEntry, Error> {
        match DirEntry::parse(&self.buf[self.offset..]) {
            Ok((size, entry)) => {
                self.offset += size;
                Ok(entry)
            }
            Err(e) => Err(e),
        }
    }
}

impl<'a> Iterator for RootDirIter<'a> {
    type Item = Result<DirEntry, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_entry() {
            Ok(entry) => match entry {
                DirEntry::EndOfDir => None,
                DirEntry::Unused => self.next(),
                _ => Some(Ok(entry)),
            },
            Err(e) => None,
        }
    }
}

#[derive(Debug)]
pub enum FatLookupError {
    BadCluster,
    ReservedCluster,
}

enum FatEntry {
    EndOfFile,
    Cluster(u32),
}

impl FatEntry {
    fn parse(
        val: u32,
        typ: FatType,
        maximum_valid_cluster: u32,
    ) -> Result<FatEntry, FatLookupError> {
        match val {
            0 | 1 => Err(FatLookupError::ReservedCluster),
            entry => {
                if entry > typ.classification_threshold() {
                    Ok(FatEntry::EndOfFile)
                } else if entry == typ.classification_threshold() {
                    Err(FatLookupError::BadCluster)
                } else {
                    Ok(FatEntry::Cluster(val))
                }
            }
        }
    }
}

struct FileAllocationTable {
    typ: FatType,
    first_sector: u32,
    sector_size: u16,
}

impl FileAllocationTable {
    pub fn new(typ: FatType, first_sector: u32, sector_size: u16) -> FileAllocationTable {
        FileAllocationTable {
            typ,
            first_sector,
            sector_size,
        }
    }

    pub fn get_value<D: Read + Seek>(&self, disk: &mut D, cluster: u32) -> u32 {
        match self.typ {
            FatType::Fat12 => {
                let fat_offset = cluster + (cluster / 2);
                let fat_sector = self.first_sector + (fat_offset / u32::from(self.sector_size));
                let entry_offset = (fat_offset % u32::from(self.sector_size)) as usize;

                let mut sector = [0u8; disk::SECTOR_SIZE];
                disk.read_exact(&mut sector);

                let value =
                    u16::from_le_bytes(sector[entry_offset..entry_offset + 2].try_into().unwrap());

                if cluster & 1 == 1 {
                    u32::from(value >> 4)
                } else {
                    u32::from(value & 0xfff)
                }
            }
            FatType::Fat16 => {
                let fat_offset = cluster * 2;
                let fat_sector = self.first_sector + (fat_offset / u32::from(self.sector_size));
                let entry_offset = (fat_offset % u32::from(self.sector_size)) as usize;

                disk.seek(SeekFrom::Start(u64::from(fat_sector)));

                let mut sector = [0u8; disk::SECTOR_SIZE];
                disk.read_exact(&mut sector);

                u32::from(u16::from_le_bytes(
                    sector[entry_offset..entry_offset + 2].try_into().unwrap(),
                ))
            }
            FatType::Fat32 => {
                let fat_offset = cluster * 4;
                let fat_sector = self.first_sector + (fat_offset / u32::from(self.sector_size));
                let entry_offset = (fat_offset % u32::from(self.sector_size)) as usize;

                disk.seek(SeekFrom::Start(u64::from(fat_sector)));

                let mut sector = [0u8; disk::SECTOR_SIZE];
                disk.read_exact(&mut sector);

                u32::from_le_bytes(sector[entry_offset..entry_offset + 4].try_into().unwrap())
                    & 0x0FFFFFFF
            }
        }
    }
}

struct Cluster {
    start_sector: u32,
    size: u8,
}

impl Cluster {
    pub fn new(start_sector: u32, size: u8) -> Cluster {
        Cluster { start_sector, size }
    }
}

struct FileIter<'a, D> {
    disk: &'a mut D,
    current_cluster: u32,
    bpb: &'a Bpb,
    fat_table: FileAllocationTable,
}

impl<D> FileIter<'_, D>
where
    D: Read + Seek,
{
    fn new(disk: &'a mut D, current_cluster: u32, bpb: &'a Bpb) -> Self {
        FileIter {
            disk,
            current_cluster,
            bpb,
            fat_table: FileAllocationTable::new(
                bpb.fat_type(),
                bpb.first_fat_sector(),
                bpb.bytes_per_sector,
            ),
        }
    }
    fn next_cluster(&mut self) -> Option<Cluster> {
        None
    }
}
