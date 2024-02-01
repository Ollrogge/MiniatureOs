use crate::const_assert;
use core::{arch::asm, default::Default, mem::size_of};

#[derive(Debug)]
#[repr(C, packed)]
#[allow(dead_code)]
pub struct VbeInfoBlock {
    signature: [u8; 4], // should be "VESA"
    version: u16,       // should be 0x0300 for VBE 3.0
    oem_string_ptr: u32,
    capabilities: u32,
    video_mode_ptr: u32,
    total_memory: u16, // number of 64KB blocks
    reserved: [u8; 512 - 0x14],
}
const_assert!(size_of::<VbeInfoBlock>() == 512, "VbeInfoBlock size");

impl Default for VbeInfoBlock {
    fn default() -> VbeInfoBlock {
        VbeInfoBlock {
            signature: [0; 4],
            version: 0,
            oem_string_ptr: 0,
            capabilities: 0,
            video_mode_ptr: 0,
            total_memory: 0,
            reserved: [0; 512 - 0x14],
        }
    }
}

impl VbeInfoBlock {
    pub fn query(&mut self) -> Result<(), u16> {
        let mut ret = 0x0;
        unsafe {
            asm!("int 0x10", inout("ax") 0x4f00u16 => ret, in("di") self, options(nostack, nomem));
        }

        match ret {
            0x004f => Ok(()),
            _ => Err(ret),
        }
    }
}
