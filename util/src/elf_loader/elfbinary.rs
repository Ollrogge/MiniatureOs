// Elf loader for x86_64 ELF files

use crate::const_assert;

static ELF_MAGIC: &str = "\x7fELF";

#[derive(Debug)]
pub enum ElfError {
    InvalidHeader(&'static str),
    Other,
}

#[repr(u8)]
enum ElfClass {
    Elf32 = 1,
    Elf64
}

#[repr(u16)]
enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
    Core
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
    Bug
}

#[repr(u16)]
enum Machine {
    X86 = 0x3,
    X86_64 = 0x3e
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
    SysV
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
    shstrndx: u16
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
        validate!(raw[0..4] == *ELF_MAGIC.as_bytes(), ElfError::InvalidHeader("Invalid Magic"));
        validate!(raw[4] == ElfClass::Elf64 as u8, ElfError::InvalidHeader("Incorrect elfclass"));
        validate!(raw[5] == Endianness::Little as u8, ElfError::InvalidHeader("Wrong endianess"));
        validate!(raw[6] == 0x1, ElfError::InvalidHeader("Incorrect ELF version"));
        validate!(raw[7] == Abi::SysV as u8, ElfError::InvalidHeader("Incorrect ABI"));
        validate!(ElfType::try_from(u16::from_le_bytes([raw[0x10], raw[0x11]])).is_ok(), ElfError::InvalidHeader("Invalid elf type"));
        validate!(u16::from_le_bytes([raw[0x12], raw[0x13]]) == Machine::X86_64 as u16, ElfError::InvalidHeader("Invalid machine"));

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
        let shentsize= u16::from_le_bytes(val16);

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
            shstrndx
        })
    }
}

pub struct ElfBinary<'a> {
    raw: &'a [u8]
}

#[cfg(test)]
mod elf_tests {
    use super::*;
    extern crate std;
    use std::fs;
    use std::path::Path;

    #[test]
    fn parse_header() {
          let file_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/elf_loader/test/hello_world");

        //panic!("FILE_PATH: {:?}", file_path);
        let e = fs::read(file_path).unwrap();
        let header = ElfHeader::new(&e);

        assert!(header.is_ok());
    }
}