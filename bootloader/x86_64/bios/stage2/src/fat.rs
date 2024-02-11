//! //! FAT file system parser
//!
//! The file system consists of three basic components:
//!
//! - Boot record
//! - File Allocation Table (FAT)
//! - Directory and data area
//!
//! Basically just a big single-linked list of clusters in a big table
//! https://wiki.osdev.org/FAT
use crate::disk::{self, SECTOR_SIZE};
use crate::disk::{Read, Seek, SeekFrom, CLUSTER_SIZE};
use core::{default::Default, ptr, str};

const ROOT_DIR_ENTRY_SIZE: usize = 0x20;

#[derive(Debug)]
pub enum FatError {
    FileNotFound,
    DirEntryError,
    FileReadError,
}

#[derive(PartialEq, Clone, Copy)]
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

/// BIOS Parameter block
/// Gives u metadata about the disk
#[derive(Debug)]
pub struct BiosParameterBlock {
    bytes_per_sector: u16,
    /// cluster = smallest unit of space allocation for files and dirs on FAT fs
    sectors_per_cluster: u8,
    /// number of sectors before first FAT
    reserved_sector_count: u16,
    /// amount of copies of FAT present on disk. Multiple used due to redundancy
    /// reasons. If one FAT is damaged it can be repaired using the backup'ed one
    fat_count: u8,
    // number of root directory entries
    root_entry_count: u16,
    /// total number of sectors on disk
    total_sectors_16: u16,
    /// total number of sectors on disk
    total_sectors_32: u32,
    /// Size of the FAT data structure in sectors
    fat16_size: u16,
    /// Size of the FAT data structure in sectors
    fat32_size: u32,
    // start cluster of root directory
    root_cluster: u32,
}

impl BiosParameterBlock {
    pub fn parse<D: Read + Seek>(disk: &mut D) -> Self {
        disk.seek(SeekFrom::Start(0));
        let mut raw = [0u8; SECTOR_SIZE];
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
            panic!("Exactly one total sector field must be zero");
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
        ((self.root_entry_count as u32 * ROOT_DIR_ENTRY_SIZE as u32)
            + (self.bytes_per_sector as u32 - 1))
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
        self.root_entry_count * ROOT_DIR_ENTRY_SIZE as u16
    }

    fn first_fat_sector(&self) -> u32 {
        self.reserved_sector_count as u32
    }

    fn first_cluster_sector(&self, cluster_number: u32) -> u32 {
        ((cluster_number - 2) * u32::from(self.sectors_per_cluster)) + self.first_data_sector()
    }

    pub fn bytes_per_cluster(&self) -> u32 {
        self.bytes_per_sector as u32 * self.sectors_per_cluster as u32
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

pub enum DirectoryEntry {
    Unused,
    EndOfDir,
    NormalDirEntry(NormalDirectoryEntry),
    LongNameDirEntry(LongNameDirectoryEntry),
}

impl DirectoryEntry {
    const END_OF_DIRECTORY: u8 = 0x0;
    const UNUSED_ENTRY: u8 = 0xe5;
    const NORMAL_ENTRY_SIZE: usize = 0x20;
    fn parse(raw: &[u8]) -> Result<(usize, DirectoryEntry), FatError> {
        if raw[0] == Self::END_OF_DIRECTORY {
            return Ok((Self::NORMAL_ENTRY_SIZE, DirectoryEntry::EndOfDir));
        } else if raw[0] == Self::UNUSED_ENTRY {
            return Ok((Self::NORMAL_ENTRY_SIZE, DirectoryEntry::Unused));
        }

        let attributes = FileAttributes(raw[11]);

        if attributes == FileAttributes::LONG_FILE_NAME {
            let mut long_name_entry = LongNameDirectoryEntry::default();
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
                    long_name_entry.first_cluster_number =
                        u32::from(u16::from_le_bytes(entry[20..22].try_into().unwrap())) << 16
                            | u32::from(u16::from_le_bytes(entry[26..28].try_into().unwrap()));

                    long_name_entry.size_in_bytes =
                        u32::from_le_bytes(entry[28..32].try_into().unwrap());

                    long_name_entry.attributes = attributes;

                    total_size = (i + 1) * Self::NORMAL_ENTRY_SIZE;
                    break;
                }
            }

            assert!(total_size != 0);

            Ok((
                total_size,
                DirectoryEntry::LongNameDirEntry(long_name_entry),
            ))
        } else {
            let mut entry = NormalDirectoryEntry::default();
            for (i, &b) in raw[0..entry.filename.len()].iter().enumerate() {
                entry.filename[i] = char::from(b);
            }
            entry.attributes = attributes;
            entry.first_cluster_number =
                u32::from(u16::from_le_bytes(raw[20..22].try_into().unwrap())) << 16
                    | u32::from(u16::from_le_bytes(raw[26..28].try_into().unwrap()));

            entry.size_in_bytes = u32::from_le_bytes(raw[28..32].try_into().unwrap());

            Ok((
                Self::NORMAL_ENTRY_SIZE,
                DirectoryEntry::NormalDirEntry(entry),
            ))
        }
    }

    pub fn eq_name(&self, name: &str) -> bool {
        match self {
            DirectoryEntry::NormalDirEntry(e) => e
                .filename
                .iter()
                .cloned()
                .take_while(|&c| c != '\0')
                .eq(name.chars()),
            DirectoryEntry::LongNameDirEntry(e) => e
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
            DirectoryEntry::NormalDirEntry(e) => e.attributes.is_set(FileAttributes::DIRECTORY),
            DirectoryEntry::LongNameDirEntry(e) => e.attributes.is_set(FileAttributes::DIRECTORY),
            _ => false,
        }
    }

    pub fn first_cluster(&self) -> u32 {
        match self {
            DirectoryEntry::NormalDirEntry(e) => e.first_cluster_number,
            DirectoryEntry::LongNameDirEntry(e) => e.first_cluster_number,
            _ => 0,
        }
    }

    pub fn file_size(&self) -> u32 {
        match self {
            DirectoryEntry::NormalDirEntry(e) => e.size_in_bytes,
            DirectoryEntry::LongNameDirEntry(e) => e.size_in_bytes,
            _ => 0,
        }
    }
}

/// Normal directory entries are described exclusively by this struct
#[derive(Default)]
pub struct NormalDirectoryEntry {
    filename: [char; 11],
    attributes: FileAttributes,
    /// Number of first cluster for this file
    pub first_cluster_number: u32,
    size_in_bytes: u32,
}

/// Long name directory entires are used only when the VFAT extension is supported.
/// With this extension, filenames can be up to 255 characters.
/// Long name directory entries are represented by a chain of 32 byte long file name entries,
/// which always end with a normal directory entry struct.
/// The filename field of the normal directory entry is ignored in this case.
pub struct LongNameDirectoryEntry {
    /// Order of this entry in the chain of long file name entries
    // TODO: handle this ?
    order: u8,
    pub filename: [char; 255],
    pub first_cluster_number: u32,
    attributes: FileAttributes,
    size_in_bytes: u32,
}

impl Default for LongNameDirectoryEntry {
    fn default() -> LongNameDirectoryEntry {
        LongNameDirectoryEntry {
            order: 0,
            filename: [0 as char; 255],
            attributes: FileAttributes::new(),
            first_cluster_number: 0,
            size_in_bytes: 0,
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

    pub fn size_in_sectors(&self) -> u32 {
        (self.size + (SECTOR_SIZE - 1) as u32) / SECTOR_SIZE as u32
    }
}

pub struct FileSystem<D> {
    disk: D,
    bpb: BiosParameterBlock,
}

impl<D: Read + Seek + Clone> FileSystem<D> {
    pub fn parse(mut disk: D) -> Self {
        Self {
            bpb: BiosParameterBlock::parse(&mut disk),
            disk,
        }
    }

    pub fn read_root_dir<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> impl Iterator<Item = Result<DirectoryEntry, FatError>> + 'a {
        assert!(buffer.len() == self.bpb.root_dir_size() as usize);

        if self.bpb.fat_type() == FatType::Fat32 {
            unimplemented!();
        }

        self.disk
            .seek(SeekFrom::Start(u64::from(self.bpb.first_root_dir_sector())));

        self.disk.read_exact(buffer);

        RootDirIter::new(buffer)
    }

    pub fn find_file_in_root_dir(&mut self, name: &str) -> Option<File> {
        // todo: somehow not hardcode this ?
        // FAT16: common to have a root directory with max 512 entries of size 32
        // If I had dynamic memory I could use bpb.root_entry_count
        let mut buffer = [0u8; 512 * ROOT_DIR_ENTRY_SIZE];
        let mut entries = self.read_root_dir(&mut buffer).filter_map(|e| e.ok());
        let entry = entries.find(|e| e.eq_name(name))?;

        if entry.is_dir() {
            None
        } else {
            Some(File::new(entry.first_cluster(), entry.file_size()))
        }
    }

    // The clusters of a file need not be right next to each other on the disk.
    // In fact it is likely that they are scattered widely throughout the disk
    // The FAT allows the operating system to follow the "chain" of clusters in a file.
    pub fn file_clusters<'a>(
        &'a mut self,
        file: &File,
    ) -> impl Iterator<Item = Result<Cluster, FatError>> + 'a {
        FileIter::new(&mut self.disk, file.start_sector, &self.bpb)
    }

    /// A file consists of a sequence of clusters. These clusters are not guaranteed to
    /// be adjacent to each other. We obtain the sector number of the first cluster
    /// from the DirectoryEntry. Afterwards we look up the start sectory of any further
    /// clusters by querying the FAT.
    pub fn try_load_file(&mut self, name: &str, dest: *mut u8) -> Result<usize, FatError> {
        let file = self
            .find_file_in_root_dir(name)
            .ok_or(FatError::FileNotFound)?;
        let mut buffer = [0u8; CLUSTER_SIZE];

        let mut disk = self.disk.clone();

        let mut sectors_read = 0x0;
        for cluster in self.file_clusters(&file) {
            let cluster = cluster?;
            disk.seek(SeekFrom::Start(u64::from(cluster.start_sector)));

            disk.read_exact(&mut buffer);
            let dest = dest.wrapping_add(sectors_read * SECTOR_SIZE);

            unsafe {
                ptr::copy_nonoverlapping(buffer.as_ptr(), dest, buffer.len());
            }

            sectors_read += 2;
        }

        // smaller and not equal because we read cluster wise and therefore
        // might read more sectors than the size of the file
        if sectors_read < file.size_in_sectors() as usize {
            Err(FatError::FileReadError)
        } else {
            Ok(file.size as usize)
        }
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

    pub fn next_entry(&mut self) -> Result<DirectoryEntry, FatError> {
        match DirectoryEntry::parse(&self.buf[self.offset..]) {
            Ok((size, entry)) => {
                self.offset += size;
                Ok(entry)
            }
            Err(e) => Err(e),
        }
    }
}

impl<'a> Iterator for RootDirIter<'a> {
    type Item = Result<DirectoryEntry, FatError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_entry() {
            Ok(entry) => match entry {
                DirectoryEntry::EndOfDir => None,
                DirectoryEntry::Unused => self.next(),
                _ => Some(Ok(entry)),
            },
            Err(e) => Some(Err(e)),
        }
    }
}

/// Entry inside the FileAllocationnTable
enum FatEntry {
    EndOfFile,
    Cluster(u32),
    BadCluster,
    ReservedCluster,
}

impl FatEntry {
    fn parse(val: u32, typ: FatType) -> FatEntry {
        match val {
            0 | 1 => FatEntry::ReservedCluster,
            entry => {
                if entry > typ.classification_threshold() {
                    FatEntry::EndOfFile
                } else if entry == typ.classification_threshold() {
                    FatEntry::BadCluster
                } else {
                    FatEntry::Cluster(val)
                }
            }
        }
    }
}

/// Table of contents of a disk. Indicates status and location of all data clusters
/// stored on the disk.
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

    pub fn get_entry<D: Read + Seek>(&self, disk: &mut D, cluster: u32) -> FatEntry {
        let val = match self.typ {
            FatType::Fat12 => {
                let fat_offset = cluster + (cluster / 2);
                let fat_sector = self.first_sector + (fat_offset / u32::from(self.sector_size));
                let entry_offset = (fat_offset % u32::from(self.sector_size)) as usize;

                disk.seek(SeekFrom::Start(u64::from(fat_sector)));

                // special case for 12 bit entries. They might not be sector aligned.
                // In this case an entry might straddle the sector-size boundary.
                // So just read two sectors in.
                let mut sector = [0u8; disk::SECTOR_SIZE * 2];
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
        };

        FatEntry::parse(val, self.typ)
    }
}

/// Smallest unit of space allocation for files and directories on a FAT fs
struct Cluster {
    start_sector: u32,
    size_in_sectors: u8,
}

impl Cluster {
    pub fn new(start_sector: u32, size: u8) -> Cluster {
        Cluster {
            start_sector,
            size_in_sectors: size,
        }
    }
}

struct FileIter<'a, D> {
    disk: &'a mut D,
    current_entry: FatEntry,
    bpb: &'a BiosParameterBlock,
    fat_table: FileAllocationTable,
}

impl<'a, D> FileIter<'a, D>
where
    D: Read + Seek,
{
    fn new(disk: &'a mut D, start_cluster: u32, bpb: &'a BiosParameterBlock) -> Self {
        FileIter {
            disk,
            current_entry: FatEntry::Cluster(start_cluster),
            bpb,
            fat_table: FileAllocationTable::new(
                bpb.fat_type(),
                bpb.first_fat_sector(),
                bpb.bytes_per_sector,
            ),
        }
    }

    fn next_cluster(&mut self) -> Result<Option<Cluster>, FatError> {
        match self.current_entry {
            FatEntry::BadCluster | FatEntry::ReservedCluster => Err(FatError::FileReadError),
            FatEntry::EndOfFile => Ok(None),
            FatEntry::Cluster(cluster_number) => {
                let cluster = Cluster::new(
                    self.bpb.first_cluster_sector(cluster_number),
                    self.bpb.sectors_per_cluster,
                );

                self.current_entry = self.fat_table.get_entry(self.disk, cluster_number);

                Ok(Some(cluster))
            }
        }
    }
}

impl<'a, D> Iterator for FileIter<'a, D>
where
    D: Read + Seek,
{
    type Item = Result<Cluster, FatError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_cluster() {
            Ok(entry) => match entry {
                Some(cluster) => Some(Ok(cluster)),
                None => None,
            },
            Err(e) => Some(Err(e)),
        }
    }
}
