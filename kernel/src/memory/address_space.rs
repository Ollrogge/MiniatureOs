use x86_64::{
    memory::{PhysicalAddress, PhysicalFrame},
    register::{Cr3, Cr3Flags},
};

pub struct AddressSpace {
    cr3: PhysicalFrame,
    cr3_flags: Cr3Flags,
}

impl AddressSpace {
    pub fn new(cr3: PhysicalFrame, cr3_flags: Cr3Flags) -> Self {
        Self { cr3, cr3_flags }
    }
}
