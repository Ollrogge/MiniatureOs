// ELF loader for x86_64 ELF files that is no_std compatible
// cheatsheet: https://gist.github.com/x0nu11byt3/bcb35c3de461e5fb66173071a2379779

use crate::const_assert;
use core::{fmt, fmt::Display};

#[derive(Debug)]
enum InvalidElfHeader {
    InvalidMagic,
    InvalidClass,
    InvalidEndianess,
    InvalidVersion,
    InvalidAbi,
    InvalidElfType,
    InvalidMachine,
}

#[derive(Debug)]
enum InvalidProgramHeader {
    UnknownSegmentFlags,
    InvalidSegmentType,
    InvalidAlignment,
    InvalidHeaderOffset,
}

#[derive(Debug)]
enum InvalidSectionHeader {
    InvalidHeaderType,
    InvalidSectionFlags,
    InvalidHeaderOffset,
}

#[derive(Debug)]
enum InvalidRelocationType {
    InvalidType,
}

#[derive(Debug)]
pub enum ElfError {
    InvalidElfHeader(InvalidElfHeader),
    InvalidProgramHeader(InvalidProgramHeader),
    InvalidSectionHeader(InvalidSectionHeader),
    InvalidRelocationType(InvalidRelocationType),
    Other,
}

#[repr(u8)]
enum ElfClass {
    Elf32 = 1,
    Elf64,
}

#[repr(u16)]
enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
    Core,
}

impl TryFrom<u16> for ElfType {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ElfType::None),
            1 => Ok(ElfType::Rel),
            2 => Ok(ElfType::Exec),
            3 => Ok(ElfType::Dyn),
            4 => Ok(ElfType::Core),
            _ => Err(""),
        }
    }
}

#[repr(u8)]
enum Endianness {
    Little = 1,
    Bug,
}

#[repr(u16)]
enum Machine {
    X86 = 0x3,
    X86_64 = 0x3e,
}

impl TryFrom<u16> for Machine {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            x if x == Machine::X86 as u16 => Ok(Machine::X86),
            x if x == Machine::X86_64 as u16 => Ok(Machine::X86),
            _ => Err(""),
        }
    }
}

#[repr(u8)]
enum Abi {
    SysV,
}

#[derive(Debug)]
struct ElfHeader {
    entry: u64,
    phoff: u64,
    shoff: u64,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

impl Display for ElfHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "ELF Header:")?;
        writeln!(f, "   Entry point address: {:#02x}", self.entry)?;
        writeln!(f, "   Start of program headers: {}", self.phoff)?;
        writeln!(f, "   Start of section headers: {}", self.shoff)?;
        writeln!(f, "   Size of program headers: {}", self.phentsize)?;
        writeln!(f, "   Number of program headers: {}", self.phnum)?;
        writeln!(f, "   Size of section headers: {}", self.shentsize)?;
        writeln!(f, "   Size of section headers: {}", self.shentsize)?;
        writeln!(f, "   Number of section headers: {}", self.shnum)?;
        write!(f, "   Section header string table index: {}", self.shstrndx)
    }
}

macro_rules! validate {
    ($condition:expr, $error:expr) => {
        if !$condition {
            return Err($error);
        }
    };
}

impl ElfHeader {
    const ELF_MAGIC: &str = "\x7fELF";

    pub fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];
        let mut val16 = [0u8; 2];

        validate!(
            raw[0..4] == *Self::ELF_MAGIC.as_bytes(),
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidMagic)
        );
        validate!(
            raw[4] == ElfClass::Elf64 as u8,
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidElfType)
        );
        validate!(
            raw[5] == Endianness::Little as u8,
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidEndianess)
        );
        validate!(
            raw[6] == 0x1,
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidVersion)
        );
        validate!(
            raw[7] == Abi::SysV as u8,
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidAbi)
        );
        validate!(
            ElfType::try_from(u16::from_le_bytes([raw[0x10], raw[0x11]])).is_ok(),
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidElfType)
        );
        validate!(
            u16::from_le_bytes([raw[0x12], raw[0x13]]) == Machine::X86_64 as u16,
            ElfError::InvalidElfHeader(InvalidElfHeader::InvalidMachine)
        );

        val64.copy_from_slice(&raw[0x18..0x20]);
        let entry = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x20..0x28]);
        let phoff = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x28..0x30]);
        let shoff = u64::from_le_bytes(val64);

        val16.copy_from_slice(&raw[0x36..0x38]);
        let phentsize = u16::from_le_bytes(val16);

        val16.copy_from_slice(&raw[0x38..0x3a]);
        let phnum = u16::from_le_bytes(val16);

        val16.copy_from_slice(&raw[0x3a..0x3c]);
        let shentsize = u16::from_le_bytes(val16);

        val16.copy_from_slice(&raw[0x3c..0x3e]);
        let shnum = u16::from_le_bytes(val16);

        val16.copy_from_slice(&raw[0x3e..0x40]);
        let shstrndx = u16::from_le_bytes(val16);

        Ok(Self {
            entry,
            phoff,
            shoff,
            phentsize,
            phnum,
            shentsize,
            shnum,
            shstrndx,
        })
    }

    pub fn program_headers<'a>(
        &self,
        raw: &'a [u8],
    ) -> Result<impl Iterator<Item = ProgramHeader> + 'a, ElfError> {
        let phnum = self.phnum;
        let phoff = self.phoff;
        let phentsize = self.phentsize;

        for i in 0..phnum as u64 {
            let offset = (phoff + i * phentsize as u64) as usize;
            if offset + phentsize as usize > raw.len() {
                return Err(ElfError::InvalidSectionHeader(
                    InvalidSectionHeader::InvalidHeaderOffset,
                ));
            }
            ProgramHeader::new(&raw[offset..])?;
        }

        Ok((0..phnum as u64).map(move |i| {
            let offset = (phoff + i * phentsize as u64) as usize;
            // We already validated these, so unwrap is safe here
            ProgramHeader::new(&raw[offset..]).unwrap()
        }))
    }

    pub fn section_headers<'a>(
        &self,
        raw: &'a [u8],
    ) -> Result<impl Iterator<Item = SectionHeader> + 'a, ElfError> {
        let shnum = self.shnum;
        let shoff = self.shoff;
        let shentsize = self.shentsize;

        for i in 0..shnum as u64 {
            let offset = (shoff + i * shentsize as u64) as usize;
            if offset + shentsize as usize > raw.len() {
                return Err(ElfError::InvalidSectionHeader(
                    InvalidSectionHeader::InvalidHeaderOffset,
                ));
            }
            SectionHeader::new(&raw[offset..])?;
        }

        Ok((0..shnum as u64).map(move |i| {
            let offset = (shoff + i * shentsize as u64) as usize;
            // We already validated these, so unwrap is safe here
            SectionHeader::new(&raw[offset..]).unwrap()
        }))
    }

    pub fn entry_point(&self) -> u64 {
        self.entry
    }
}

#[derive(Debug, PartialEq)]
#[repr(u32)]
enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phdr,
    Tls,
    // GCC .eh_frame_hdr segment
    GnuEhFrame,
    // Indicates stack executability
    GnuStack,
    // Read-only after relocation
    GnuRelro,
}

impl TryFrom<u32> for SegmentType {
    type Error = ElfError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SegmentType::Null),
            1 => Ok(SegmentType::Load),
            2 => Ok(SegmentType::Dynamic),
            3 => Ok(SegmentType::Interp),
            4 => Ok(SegmentType::Note),
            5 => Ok(SegmentType::Shlib),
            6 => Ok(SegmentType::Phdr),
            7 => Ok(SegmentType::Tls),
            0x6474e550 => Ok(SegmentType::GnuEhFrame),
            0x6474e551 => Ok(SegmentType::GnuStack),
            0x6474e552 => Ok(SegmentType::GnuRelro),
            _ => Err(ElfError::InvalidProgramHeader(
                InvalidProgramHeader::InvalidSegmentType,
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentFlags {
    flags: u32,
}

impl SegmentFlags {
    const EXECUTABLE: u32 = 1;
    const WRITABLE: u32 = 2;
    const READABLE: u32 = 4;

    pub fn new(flags: u32) -> Result<Self, ElfError> {
        let known_flags = Self::EXECUTABLE | Self::WRITABLE | Self::READABLE;
        let unknown_flags = flags & !known_flags;

        if unknown_flags != 0 {
            return Err(ElfError::InvalidProgramHeader(
                InvalidProgramHeader::UnknownSegmentFlags,
            ));
        }

        Ok(Self { flags })
    }

    pub fn is_executable(&self) -> bool {
        (self.flags & Self::EXECUTABLE) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.flags & Self::WRITABLE) != 0
    }

    pub fn is_readable(&self) -> bool {
        (self.flags & Self::READABLE) != 0
    }
}

impl fmt::Display for SegmentFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;

        if self.is_readable() {
            write!(f, "R")?;
            first = false;
        }

        if self.is_writable() {
            write!(f, "W")?;
            first = false;
        }

        if self.is_executable() {
            write!(f, "X")?;
            first = false;
        }

        if first {
            write!(f, "NONE")?;
        }

        Ok(())
    }
}

impl From<u32> for SegmentFlags {
    fn from(flags: u32) -> Self {
        SegmentFlags { flags }
    }
}

pub struct ProgramHeader {
    /// type of segment
    typ: SegmentType,
    /// segment-dependent flags
    flags: SegmentFlags,
    /// offset of segment in file
    offset: u64,
    /// virtual address of segment
    vaddr: u64,
    /// physical address of segment
    paddr: u64,
    /// size of segment in file (bytes)
    filesz: u64,
    /// size of segment in memory (bytes)
    memsz: u64,
    /// alignment. Must be power of two, else no alignment
    align: u64,
}
const_assert!(size_of::<ProgramHeader>() == 0x38);

impl Display for ProgramHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ProgramHeader {{")?;
        writeln!(f, "    type:      {:?}", self.typ)?;
        writeln!(f, "    plags:     {}", self.flags)?;
        writeln!(f, "    offset:    {:?}", self.offset)?;
        writeln!(f, "    vaddr:     {:#016x}", self.vaddr)?;
        writeln!(f, "    paddr:     {:#016x}", self.paddr)?;
        writeln!(f, "    filesz:    {:#016x}", self.filesz)?;
        writeln!(f, "    memsz:     {:#016x}", self.memsz)?;
        writeln!(f, "    align:     {:#016x}", self.align)?;
        write!(f, "}}")
    }
}

impl ProgramHeader {
    fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];
        let mut val32 = [0u8; 4];

        val32.copy_from_slice(&raw[0x0..0x4]);
        let p_type: SegmentType = u32::from_le_bytes(val32).try_into()?;

        val32.copy_from_slice(&raw[0x4..0x8]);
        let p_flags: SegmentFlags = u32::from_le_bytes(val32).into();

        val64.copy_from_slice(&raw[0x8..0x10]);
        let p_offset: u64 = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x10..0x18]);
        let p_vaddr: u64 = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x18..0x20]);
        let p_paddr: u64 = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x20..0x28]);
        let p_filesz: u64 = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x28..0x30]);
        let p_memsz: u64 = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x30..0x38]);
        let p_align: u64 = u64::from_le_bytes(val64);

        if p_align != 0 && !p_align.is_power_of_two() {
            return Err(ElfError::InvalidProgramHeader(
                InvalidProgramHeader::InvalidAlignment,
            ));
        }

        Ok(Self {
            typ: p_type,
            flags: p_flags,
            offset: p_offset,
            vaddr: p_vaddr,
            paddr: p_paddr,
            filesz: p_filesz,
            memsz: p_memsz,
            align: p_align,
        })
    }

    pub fn is_loadable(&self) -> bool {
        self.typ == SegmentType::Load
    }

    pub fn is_tls(&self) -> bool {
        self.typ == SegmentType::Tls
    }

    pub fn virtual_addr(&self) -> u64 {
        self.vaddr
    }

    pub fn mem_size(&self) -> u64 {
        self.memsz
    }

    pub fn flags(&self) -> SegmentFlags {
        self.flags
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn file_size(&self) -> u64 {
        self.filesz
    }
}

#[derive(Debug, PartialEq)]
#[repr(u32)]
pub enum SectionHeaderType {
    /// Uused section header
    Null,
    /// Program data
    Progbits,
    /// Symbol table
    Symtab,
    /// String table
    Strtab,
    /// Relocation entries with addends
    Rela,
    /// Symbol hash table
    ShtHash,
    /// Dynamic linking information
    Dynamic,
    /// Notes
    Note,
    /// Program space with no data (bss)
    Nobits,
    /// Relocation entries without addends
    Rel,
    /// Reserved
    Shlib,
    /// Dynamic linking symbol table
    Dynsym,
    /// Array of constructors
    InitArray,
    /// Array of destructors
    FiniArray,
    /// Array of pre-constructors
    PreinitArray,
    /// Section group
    Group,
    /// Extended section indices
    SymtabShndx,
    GnuHash,
    /// Number of defined types
    Num,
}

impl TryFrom<u32> for SectionHeaderType {
    type Error = ElfError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Null),
            1 => Ok(Self::Progbits),
            2 => Ok(Self::Symtab),
            3 => Ok(Self::Strtab),
            4 => Ok(Self::Rela),
            5 => Ok(Self::ShtHash),
            6 => Ok(Self::Dynamic),
            7 => Ok(Self::Note),
            8 => Ok(Self::Nobits),
            9 => Ok(Self::Rel),
            10 => Ok(Self::Shlib),
            11 => Ok(Self::Dynsym),
            14 => Ok(Self::InitArray),
            15 => Ok(Self::FiniArray),
            16 => Ok(Self::PreinitArray),
            17 => Ok(Self::Group),
            18 => Ok(Self::SymtabShndx),
            0x6ffffff6 => Ok(Self::GnuHash),
            _ => Err(ElfError::InvalidSectionHeader(
                InvalidSectionHeader::InvalidHeaderType,
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionFlags {
    flags: u64,
}

impl SectionFlags {
    // Writable
    const WRITE: u64 = 0x1;
    const ALLOC: u64 = 0x2;
    const EXECINSTR: u64 = 0x4;
    const MERGE: u64 = 0x10;
    const STRINGS: u64 = 0x20;
    const INFO_LINK: u64 = 0x40;
    const LINK_ORDER: u64 = 0x80;
    const OS_NONCONFORMING: u64 = 0x100;
    const GROUP: u64 = 0x200;
    const TLS: u64 = 0x400;

    pub fn new(flags: u64) -> Result<Self, ElfError> {
        let known_flags = Self::WRITE
            | Self::ALLOC
            | Self::EXECINSTR
            | Self::MERGE
            | Self::STRINGS
            | Self::INFO_LINK
            | Self::LINK_ORDER
            | Self::OS_NONCONFORMING
            | Self::GROUP
            | Self::TLS;
        let unknown_flags = flags & !known_flags;

        if unknown_flags != 0 {
            return Err(ElfError::InvalidSectionHeader(
                InvalidSectionHeader::InvalidSectionFlags,
            ));
        }

        Ok(Self { flags })
    }
}

impl From<u64> for SectionFlags {
    fn from(flags: u64) -> Self {
        SectionFlags { flags }
    }
}

/// Represents an ELF section header which provides information about a section in the ELF file.
pub struct SectionHeader {
    /// Offset (in bytes) to the section name string in the section header string table.
    /// This is an index into the .shstrtab section.
    name_off: u32,

    /// Specifies the type of this section, indicating its contents and semantics.
    /// Different types have different rules for how their data is interpreted.
    typ: SectionHeaderType,

    /// Bit flags that specify section attributes such as writability,
    /// executability, and allocation requirements.
    flags: SectionFlags,

    /// Virtual address of the section in memory during process execution.
    /// If the section is not loaded into memory, this field is zero.
    vaddr: u64,

    /// Offset (in bytes) from the beginning of the file to the first byte of the section.
    /// For SHT_NOBITS sections, this points to where the section would begin, even though it occupies no space.
    offset: u64,

    /// Size of the section in bytes. For SHT_NOBITS sections, this may not occupy any space in the file
    /// but indicates how many bytes the section will occupy in memory.
    size: u64,

    /// Section header table index link, whose interpretation depends on the section type.
    /// For symbol tables, this is the section header index of the associated string table.
    /// For relocation sections, this is the section header index of the associated symbol table.
    link: u32,

    /// Extra information whose interpretation depends on the section type.
    /// For symbol tables, this is the index of the first non-local symbol.
    /// For relocation sections, this is the section header index of the section to which the relocations apply.
    info: u32,

    /// Required alignment of the section in memory, expressed as a power of 2.
    /// 0 or 1 means the section has no alignment constraints.
    addr_align: u64,

    /// Size (in bytes) of each entry, for sections that contain fixed-size entries.
    /// For sections without fixed-size entries (like string tables), this value is 0.
    entsize: u64,
}
const_assert!(size_of::<SectionHeader>() == 0x40);

impl Display for SectionHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SectionHeader {{")?;
        writeln!(f, "    name_off:  {:#016x}", self.name_off)?;
        writeln!(f, "    type:      {:?}", self.typ)?;
        writeln!(f, "    flags:     {:?}", self.flags)?;
        writeln!(f, "    vaddr:     {:#016x}", self.vaddr)?;
        writeln!(f, "    offset:    {:#016x}", self.offset)?;
        writeln!(f, "    size:      {:#016x}", self.size)?;
        writeln!(f, "    link:      {:#016x}", self.link)?;
        writeln!(f, "    info:      {:#016x}", self.info)?;
        writeln!(f, "    addr_align:{:#016x}", self.addr_align)?;
        writeln!(f, "    entsize:   {:#016x}", self.entsize)?;
        write!(f, "}}")
    }
}

impl SectionHeader {
    pub fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];
        let mut val32 = [0u8; 4];

        val32.copy_from_slice(&raw[0x0..0x4]);
        let sh_name: u32 = u32::from_le_bytes(val32);

        val32.copy_from_slice(&raw[0x4..0x8]);
        let sh_type: SectionHeaderType = u32::from_le_bytes(val32).try_into()?;

        val64.copy_from_slice(&raw[0x8..0x10]);
        let sh_flags: SectionFlags = u64::from_le_bytes(val64).into();

        val64.copy_from_slice(&raw[0x10..0x18]);
        let sh_addr = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x18..0x20]);
        let sh_offset = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x20..0x28]);
        let sh_size = u64::from_le_bytes(val64);

        val32.copy_from_slice(&raw[0x28..0x2c]);
        let sh_link = u32::from_le_bytes(val32);

        val32.copy_from_slice(&raw[0x2c..0x30]);
        let sh_info = u32::from_le_bytes(val32);

        val64.copy_from_slice(&raw[0x30..0x38]);
        let sh_addralign = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x38..0x40]);
        let sh_entsize = u64::from_le_bytes(val64);

        Ok(Self {
            name_off: sh_name,
            typ: sh_type,
            flags: sh_flags,
            vaddr: sh_addr,
            offset: sh_offset,
            size: sh_size,
            link: sh_link,
            info: sh_info,
            addr_align: sh_addralign,
            entsize: sh_entsize,
        })
    }

    pub fn contains_relocation_entries(&self) -> bool {
        self.typ == SectionHeaderType::Rel || self.typ == SectionHeaderType::Rela
    }
}

//  R_AMD64_RELATIVE => B + A
// S: The value of the symbol referenced by the relocation. This is the final address of the symbol after linking and loading.

// B: The base address at which the shared object is loaded into memory during execution. The virtual
// address where the shared object will be loaded.

// A: The addend used to compute the value of the relocatable field. This is the explicit addend stored in the relocation entry (for RELA relocations).

#[derive(Debug)]
#[repr(u32)]
pub enum RelocationType {
    Amd64None,
    Amd6464,
    Amd64Pc32,
    Amd64Got32,
    Amd64Plt32,
    Amd64Copy,
    Amd64GlobDat,
    Amd64JumpSlot,
    // B + A = virtual base + addend
    Amd64Relative,
    Amd64GotPcrel,
    Amd64TlsGd,
    Amd64TlsLd,
    Amd64DtpOff32,
    Amd64GotTpOff,
    Amd64TpOff64,
    Amd64Size32,
    Amd64Size64,
}

impl TryFrom<u32> for RelocationType {
    type Error = ElfError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RelocationType::Amd64None),
            1 => Ok(RelocationType::Amd6464),
            2 => Ok(RelocationType::Amd64Pc32),
            3 => Ok(RelocationType::Amd64Got32),
            4 => Ok(RelocationType::Amd64Plt32),
            5 => Ok(RelocationType::Amd64Copy),
            6 => Ok(RelocationType::Amd64GlobDat),
            7 => Ok(RelocationType::Amd64JumpSlot),
            8 => Ok(RelocationType::Amd64Relative),
            9 => Ok(RelocationType::Amd64GotPcrel),
            10 => Ok(RelocationType::Amd64TlsGd),
            11 => Ok(RelocationType::Amd64TlsLd),
            12 => Ok(RelocationType::Amd64DtpOff32),
            13 => Ok(RelocationType::Amd64GotTpOff),
            14 => Ok(RelocationType::Amd64TpOff64),
            15 => Ok(RelocationType::Amd64Size32),
            16 => Ok(RelocationType::Amd64Size64),
            _ => Err(ElfError::InvalidRelocationType(
                InvalidRelocationType::InvalidType,
            )),
        }
    }
}

// Rel = Relocation
// Rela = Relocation with addend
pub struct RelocationEntry {
    pub rtype: RelocationType,
    pub offset: u64,
    pub symbol_tbl_idx: u32,
    pub addend: Option<u64>,
}

impl RelocationEntry {
    pub fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];

        val64.copy_from_slice(&raw[0x0..0x8]);
        let r_offset = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x8..0x10]);
        let r_info = u64::from_le_bytes(val64);

        Ok(RelocationEntry {
            offset: r_offset,
            rtype: RelocationType::try_from((r_info & 0xffffffff) as u32)?,
            symbol_tbl_idx: (r_info >> 32) as u32,
            addend: None,
        })
    }

    pub fn new_rela(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];

        val64.copy_from_slice(&raw[0x0..0x8]);
        let r_offset = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x8..0x10]);
        let r_info = u64::from_le_bytes(val64);

        val64.copy_from_slice(&raw[0x10..0x18]);
        let r_addend = u64::from_le_bytes(val64);

        Ok(RelocationEntry {
            offset: r_offset,
            rtype: RelocationType::try_from((r_info & 0xffffffff) as u32)?,
            symbol_tbl_idx: (r_info >> 32) as u32,
            addend: Some(r_addend),
        })
    }
}

impl Display for RelocationEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "RelocationEntry {{")?;
        writeln!(f, "    relocation type:  {:?}", self.rtype)?;
        writeln!(f, "    offset:     {:#016x}", self.offset)?;
        writeln!(
            f,
            "    symbol table index:    {:#016x}",
            self.symbol_tbl_idx
        )?;
        writeln!(f, "    addend:      {:?}", self.addend)?;
        write!(f, "}}")
    }
}

pub struct ElfBinary<'a> {
    raw: &'a [u8],
    header: ElfHeader,
}

impl<'a> ElfBinary<'a> {
    pub fn new(raw: &'a [u8]) -> Result<Self, ElfError> {
        let header = ElfHeader::new(&raw).expect("Header parsing failed");

        Ok(Self { raw, header })
    }

    pub fn entry_point(&self) -> u64 {
        self.header.entry_point()
    }

    pub fn section_headers(&self) -> Result<impl Iterator<Item = SectionHeader> + 'a, ElfError> {
        self.header.section_headers(self.raw)
    }

    pub fn program_headers(&self) -> Result<impl Iterator<Item = ProgramHeader> + 'a, ElfError> {
        self.header.program_headers(self.raw)
    }

    pub fn relocation_entries<'s>(
        &'s self,
    ) -> Result<impl Iterator<Item = RelocationEntry> + 's, ElfError> {
        for sh in self
            .section_headers()?
            .filter(|sh| sh.contains_relocation_entries())
        {
            let entry_count = sh.size / sh.entsize;
            for i in 0..entry_count {
                let rel_raw = &self.raw[(sh.offset + i * sh.entsize) as usize..];
                match sh.typ {
                    SectionHeaderType::Rel => RelocationEntry::new(rel_raw)?,
                    SectionHeaderType::Rela => RelocationEntry::new_rela(rel_raw)?,
                    _ => unreachable!(),
                };
            }
        }

        Ok(self
            .section_headers()?
            .filter(|sh| sh.contains_relocation_entries())
            .flat_map(move |sh| {
                let entry_count = sh.size / sh.entsize;
                (0..entry_count).map(move |i| {
                    let rel_raw = &self.raw[(sh.offset + i * sh.entsize) as usize..];
                    match sh.typ {
                        SectionHeaderType::Rel => RelocationEntry::new(rel_raw).unwrap(),
                        SectionHeaderType::Rela => RelocationEntry::new_rela(rel_raw).unwrap(),
                        _ => unreachable!(),
                    }
                })
            }))
    }
}

#[cfg(test)]
mod elf_tests {
    use super::*;
    extern crate alloc;
    extern crate std;
    use alloc::vec::Vec;
    use std::{fs, path::Path, println};

    // Better to keep test resources in a dedicated test directory
    const TEST_FILE_PATH: &'static str = "src/elf_loader/test/test";
    const TEST_KERNEL_PATH: &'static str = "src/elf_loader/test/kernel";

    fn load_test_file(path: &'static str) -> Vec<u8> {
        let file_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        fs::read(file_path).expect("Failed to read test file")
    }

    #[test]
    fn test_parse_header() {
        let raw = load_test_file(TEST_FILE_PATH);

        let eh = ElfHeader::new(&raw).expect("Header parsing failed");
        println!("Parsed output: {}", eh);
    }

    #[test]
    fn test_parse_program_headers() {
        let raw = load_test_file(TEST_KERNEL_PATH);

        let header = ElfHeader::new(&raw).expect("Header parsing failed");
        for ph in header.program_headers(&raw).unwrap() {
            println!("Parsed output: {}", ph);
        }
    }

    #[test]
    fn test_parse_section_headers() {
        let raw = load_test_file(TEST_KERNEL_PATH);

        let header = ElfHeader::new(&raw).expect("Header parsing failed");
        for sh in header.section_headers(&raw).unwrap() {
            if sh.entsize > 0 {
                assert_eq!(sh.size % sh.entsize, 0);
            }
            println!("Parsed output: {}", sh);
        }
    }

    #[test]
    fn test_parse_relocation_entry() {
        let raw = load_test_file(TEST_KERNEL_PATH);

        let elf = ElfBinary::new(&raw).unwrap();

        let relocation_entries = elf
            .relocation_entries()
            .expect("Failed to parse relocation entries");

        for e in relocation_entries {
            println!("{}", e);
        }
    }
}
