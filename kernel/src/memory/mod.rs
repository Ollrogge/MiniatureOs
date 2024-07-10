pub mod address_space;
pub mod manager;
pub mod region;
pub mod virtual_memory_object;

#[derive(Debug)]
pub enum MemoryError {
    OutOfPhysicalMemory,
    OutOfVirtualMemory,
    InvalidSize,
    InvalidRange,
    InvalidRegion,
    Other,
}
