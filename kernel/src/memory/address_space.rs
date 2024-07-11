use x86_64::{
    memory::{PhysicalAddress, PhysicalFrame},
    register::{Cr3, Cr3Flags},
};

#[derive(Clone, Copy)]
pub struct AddressSpace {
    pub cr3: u64,
}

impl AddressSpace {
    pub fn new(cr3: u64) -> Self {
        Self { cr3 }
    }
}
