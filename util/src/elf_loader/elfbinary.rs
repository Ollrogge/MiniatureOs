// ELF loader for x86_64 ELF files

extern crate alloc;
use crate::const_assert;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::{fmt, fmt::Display};

#[derive(Debug)]
pub enum ElfError {
    InvalidElfHeader(String),
    InvalidProgramHeader(String),
    InvalidSectionHeader(String),
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
            ElfError::InvalidElfHeader("Invalid Magic".to_string())
        );
        validate!(
            raw[4] == ElfClass::Elf64 as u8,
            ElfError::InvalidElfHeader("Incorrect elfclass".to_string())
        );
        validate!(
            raw[5] == Endianness::Little as u8,
            ElfError::InvalidElfHeader("Wrong endianess".to_string())
        );
        validate!(
            raw[6] == 0x1,
            ElfError::InvalidElfHeader("Incorrect ELF version".to_string())
        );
        validate!(
            raw[7] == Abi::SysV as u8,
            ElfError::InvalidElfHeader("Incorrect ABI".to_string())
        );
        validate!(
            ElfType::try_from(u16::from_le_bytes([raw[0x10], raw[0x11]])).is_ok(),
            ElfError::InvalidElfHeader("Invalid elf type".to_string())
        );
        validate!(
            u16::from_le_bytes([raw[0x12], raw[0x13]]) == Machine::X86_64 as u16,
            ElfError::InvalidElfHeader("Invalid machine".to_string())
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
}

#[derive(Debug)]
#[repr(u32)]
enum SegmentType {
    NULL,
    LOAD,
    DYNAMIC,
    INTERP,
    SHLIB,
    PHDR,
    TLS,
}

impl TryFrom<u32> for SegmentType {
    type Error = ElfError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SegmentType::NULL),
            1 => Ok(SegmentType::LOAD),
            2 => Ok(SegmentType::DYNAMIC),
            3 => Ok(SegmentType::INTERP),
            4 => Ok(SegmentType::SHLIB),
            5 => Ok(SegmentType::PHDR),
            6 => Ok(SegmentType::TLS),
            _ => Err(ElfError::InvalidProgramHeader(format!(
                "Unknown segment type: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SegmentFlags {
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
            return Err(ElfError::InvalidProgramHeader(format!(
                "Unknown segment flags: {:#x}",
                unknown_flags
            )));
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

struct ProgramHeader {
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
            return Err(ElfError::InvalidProgramHeader(format!(
                "Incorrect p_align value: {}",
                p_align
            )));
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

    // program header table is found at offset phoff and consists of phnum entries with size phentsize
    fn parse_program_header_table(
        raw: &[u8],
        phnum: u16,
        phentsize: u16,
    ) -> Result<Vec<Self>, ElfError> {
        let Some(table_size) = phentsize.checked_mul(phnum) else {
            return Err(ElfError::InvalidProgramHeader(
                "Program header size overflow".to_string(),
            ));
        };

        if table_size as usize > raw.len() {
            return Err(ElfError::InvalidProgramHeader(
                "Program heaProgram header bigger than remaining data".to_string(),
            ));
        }

        (0..phnum)
            .map(|i| {
                let start = (i as usize) * (phentsize as usize);
                ProgramHeader::new(&raw[start..start + (phentsize as usize)])
            })
            .collect()
    }
}

#[derive(Debug)]
#[repr(u32)]
pub enum SectionHeaderType {
    /// Uused section header
    NULL,
    /// Program data
    PROGBITS,
    /// Symbol table
    SYMTAB,
    /// String table
    STRTAB,
    /// Relocation entries with addends
    RELA,
    /// Symbol hash table
    SHT_HASH,
    /// Dynamic linking information
    DYNAMIC,
    /// Notes
    NOTE,
    /// Program space with no data (bss)
    NOBITS,
    /// Relocation entries without addends
    REL,
    /// Reserved
    SHLIB,
    /// Dynamic linking symbol table
    DYNSYM,
    /// Array of constructors
    INIT_ARRAY,
    /// Array of destructors
    FINI_ARRAY,
    /// Array of pre-constructors
    PREINIT_ARRAY,
    /// Section group
    GROUP,
    /// Extended section indices
    SYMTAB_SHNDX,
    /// Number of defined types
    NUM,
}

impl TryFrom<u32> for SectionHeaderType {
    type Error = ElfError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NULL),
            1 => Ok(Self::PROGBITS),
            2 => Ok(Self::SYMTAB),
            3 => Ok(Self::STRTAB),
            4 => Ok(Self::RELA),
            5 => Ok(Self::SHT_HASH),
            6 => Ok(Self::DYNAMIC),
            7 => Ok(Self::NOTE),
            8 => Ok(Self::NOBITS),
            9 => Ok(Self::REL),
            10 => Ok(Self::SHLIB),
            11 => Ok(Self::DYNSYM),
            14 => Ok(Self::INIT_ARRAY),
            15 => Ok(Self::FINI_ARRAY),
            16 => Ok(Self::PREINIT_ARRAY),
            17 => Ok(Self::GROUP),
            18 => Ok(Self::SYMTAB_SHNDX),
            _ => Err(ElfError::InvalidSectionHeader(format!(
                "Unknown segment type: {}",
                value
            ))),
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
            return Err(ElfError::InvalidSectionHeader(format!(
                "Unknown segment flags: {:#x}",
                unknown_flags
            )));
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
}

pub struct ElfBinary<'a> {
    raw: &'a [u8],
}

#[cfg(test)]
mod elf_tests {
    use super::*;
    extern crate std;
    use std::{fs, path::Path, println};

    // Better to keep test resources in a dedicated test directory
    const TEST_FILE_PATH: &str = "src/elf_loader/test/test";

    fn load_test_file() -> Vec<u8> {
        let file_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TEST_FILE_PATH);
        fs::read(file_path).expect("Failed to read test file")
    }

    #[test]
    fn parse_header() {
        let raw = load_test_file();

        // Test both success and error cases
        match ElfHeader::new(&raw) {
            Ok(header) => {
                println!("Parsed output: {}", header);
            }
            Err(e) => {
                panic!("Failed to parse header: {:?}", e);
            }
        }
    }

    #[test]
    fn parse_program_headers() {
        let raw = load_test_file();

        let header = ElfHeader::new(&raw).expect("Header parsing failed");
        for i in 0..header.phnum as u64 {
            match ProgramHeader::new(&raw[(header.phoff + i * header.phentsize as u64) as usize..])
            {
                Ok(program_header) => {
                    println!("Parsed output: {}", program_header);
                }
                Err(e) => {
                    panic!("Failed to parse program header: {:?}", e);
                }
            }
        }
    }

    #[test]
    fn parse_section_headers() {
        let raw = load_test_file();

        let header = ElfHeader::new(&raw).expect("Header parsing failed");
        for i in 0..header.shnum as u64 {
            match SectionHeader::new(&raw[(header.shoff + i * header.shentsize as u64) as usize..])
            {
                Ok(section_header) => {
                    println!("Parsed output: {}", section_header);
                }
                Err(e) => {
                    panic!("Failed to parse program header: {:?}", e);
                }
            }
        }
    }
}
