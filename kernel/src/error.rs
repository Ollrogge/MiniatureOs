use crate::memory::MemoryError;
use core::{fmt, iter::Map};
use x86_64::paging::{MappingError, TranslationError};

#[derive(Debug)]
pub enum KernelError {
    MemoryError(MemoryError),
    // TODO: merge the two below
    MappingError(MappingError),
    TranslationError(TranslationError),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::MemoryError(e) => write!(f, "Allocation error: {:?}", e),
            KernelError::MappingError(e) => write!(f, "Paging mapping error: {:?}", e),
            KernelError::TranslationError(e) => write!(f, "Paging translation error: {:?}", e),
        }
    }
}

impl core::error::Error for KernelError {}

impl From<MemoryError> for KernelError {
    fn from(error: MemoryError) -> Self {
        KernelError::MemoryError(error)
    }
}

impl From<MappingError> for KernelError {
    fn from(error: MappingError) -> Self {
        KernelError::MappingError(error)
    }
}

impl From<TranslationError> for KernelError {
    fn from(error: TranslationError) -> Self {
        KernelError::TranslationError(error)
    }
}
