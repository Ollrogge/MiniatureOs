use crate::memory::MemoryError;
use core::{
    fmt::{self, write},
    iter::Map,
};
use x86_64::paging::{MappingError, TranslationError, UnmappingError};

#[derive(Debug)]
pub enum KernelError {
    MemoryError(MemoryError),
    // TODO: merge the two below
    PagingMappingError(MappingError),
    PagingTranslationError(TranslationError),
    PagingUnmappingError(UnmappingError),
    Other,
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::MemoryError(e) => write!(f, "Allocation error: {:?}", e),
            KernelError::PagingMappingError(e) => write!(f, "Paging mapping error: {:?}", e),
            KernelError::PagingTranslationError(e) => {
                write!(f, "Paging translation error: {:?}", e)
            }
            KernelError::PagingUnmappingError(e) => write!(f, "Unmapping error: {:?}", e),
            KernelError::Other => write!(f, "Other error"),
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
        KernelError::PagingMappingError(error)
    }
}

impl From<UnmappingError> for KernelError {
    fn from(error: UnmappingError) -> Self {
        KernelError::PagingUnmappingError(error)
    }
}

impl From<TranslationError> for KernelError {
    fn from(error: TranslationError) -> Self {
        KernelError::PagingTranslationError(error)
    }
}
