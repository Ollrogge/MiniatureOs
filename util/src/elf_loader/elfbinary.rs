// ELF loader for x86_64 ELF files

extern crate alloc;
use alloc::vec::Vec;
use core::fmt::Display;

static ELF_MAGIC: &str = "\x7fELF";

#[derive(Debug)]
pub enum ElfError {
    InvalidElfHeader(&'static str),
    InvalidProgramHeader(&'static str),
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
    pub fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];
        let mut val16 = [0u8; 2];
        validate!(
            raw[0..4] == *ELF_MAGIC.as_bytes(),
            ElfError::InvalidElfHeader("Invalid Magic")
        );
        validate!(
            raw[4] == ElfClass::Elf64 as u8,
            ElfError::InvalidElfHeader("Incorrect elfclass")
        );
        validate!(
            raw[5] == Endianness::Little as u8,
            ElfError::InvalidElfHeader("Wrong endianess")
        );
        validate!(
            raw[6] == 0x1,
            ElfError::InvalidElfHeader("Incorrect ELF version")
        );
        validate!(
            raw[7] == Abi::SysV as u8,
            ElfError::InvalidElfHeader("Incorrect ABI")
        );
        validate!(
            ElfType::try_from(u16::from_le_bytes([raw[0x10], raw[0x11]])).is_ok(),
            ElfError::InvalidElfHeader("Invalid elf type")
        );
        validate!(
            u16::from_le_bytes([raw[0x12], raw[0x13]]) == Machine::X86_64 as u16,
            ElfError::InvalidElfHeader("Invalid machine")
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
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SegmentType::NULL),
            1 => Ok(SegmentType::LOAD),
            2 => Ok(SegmentType::DYNAMIC),
            3 => Ok(SegmentType::INTERP),
            4 => Ok(SegmentType::SHLIB),
            5 => Ok(SegmentType::PHDR),
            6 => Ok(SegmentType::TLS),
            _ => Err("Unknown segment type"),
        }
    }
}

enum SegmentFlags {
    EXECUTABLE,
    WRITABLE,
    READABLE,
}

struct ProgramHeader {
    typ: SegmentType,
    flags: SegmentFlags,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

impl ProgramHeader {
    fn new(raw: &[u8]) -> Result<Self, ElfError> {
        let mut val64 = [0u8; 8];
        let mut val32 = [0u8; 4];
    }

    // program header table is found at offset phoff and consists of phnum entries with size phentsize
    fn parse_program_header_table(
        raw: &[u8],
        phnum: u16,
        phentsize: u16,
    ) -> Result<Vec<Self>, ElfError> {
        let Some(table_size) = phentsize.checked_mul(phnum) else {
            return Err(ElfError::InvalidProgramHeader(
                "Program header size overflow",
            ));
        };

        if table_size as usize > raw.len() {
            return Err(ElfError::InvalidProgramHeader(
                "Program heaProgram header bigger than remaining data",
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

pub struct ElfBinary<'a> {
    raw: &'a [u8],
}

#[cfg(test)]
mod elf_tests {
    use super::*;
    extern crate std;
    use std::{fs, path::Path, println};

    #[test]
    fn parse_header() {
        let file_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/elf_loader/test/hello_world");

        //panic!("FILE_PATH: {:?}", file_path);
        let e = fs::read(file_path).unwrap();
        let header = ElfHeader::new(&e);

        assert!(header.is_ok());
        println!("Parsed output: {}", header.unwrap());
    }
}
