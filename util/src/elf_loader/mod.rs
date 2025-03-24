pub mod elf;

use elf::{ElfBinary, ElfError, ProgramHeader, RelocationEntry};

pub trait ElfLoader {
    fn allocate(&mut self, program_header: ProgramHeader);
    fn relocate(&mut self, relocation: RelocationEntry);
    fn tls(&mut self);
}

impl<'a> ElfBinary<'a> {
    pub fn load(&self, loader: &mut dyn ElfLoader) -> Result<(), ElfError> {
        // Named functions in Rust have unique types that include their identity.
        // We must cast to fn(&&ProgramHeader) -> bool to match the trait's
        // expected function pointer type.
        if self.program_headers()?.filter(|ph| ph.is_tls()).count() > 0 {
            todo!("Implement support for tls");
        }

        self.program_headers()?
            .filter(|p| p.is_loadable())
            .for_each(|p| loader.allocate(p));

        self.relocation_entries()?
            .for_each(|rel| loader.relocate(rel));

        Ok(())
    }
}
