use crate::memory::MemoryError;
use core::fmt;

#[derive(Debug)]
pub enum KernelError {
    MemoryError(MemoryError),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::MemoryError(e) => write!(f, "Allocation error: {:?}", e),
            // Handle other error types here
        }
    }
}

impl core::error::Error for KernelError {}

impl From<MemoryError> for KernelError {
    fn from(error: MemoryError) -> Self {
        KernelError::MemoryError(error)
    }
}
