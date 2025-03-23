pub mod elfbinary;

use core::{iter::Filter, slice::Iter};
use elfbinary::{ElfBinary, ElfError, ProgramHeader, RelocationEntry};

pub type LoadableSegments<'a> = Filter<Iter<'a, ProgramHeader>, fn(&&ProgramHeader) -> bool>;

pub trait ElfLoader {
    // fn allocate(&self, program_headers: LoadableSegments<'_>);
    fn allocate(&self, program_header: ProgramHeader);
    fn relocate(&self, relocation: RelocationEntry);
    fn tls(&self);
}

impl<'a> ElfBinary<'a> {
    pub fn load(&self, loader: &dyn ElfLoader) -> Result<(), ElfError> {
        // Named functions in Rust have unique types that include their identity.
        // We must cast to fn(&&ProgramHeader) -> bool to match the trait's
        // expected function pointer type.

        self.program_headers()?
            .filter(|p| p.is_loadable())
            .for_each(|p| loader.allocate(p));

        self.relocation_entries()?
            .for_each(|rel| loader.relocate(rel));

        Ok(())
    }
}
