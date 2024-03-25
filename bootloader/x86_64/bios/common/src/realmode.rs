#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RealModePointer(pub u32);

impl RealModePointer {
    pub fn segment(&self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub fn offset(&self) -> u16 {
        self.0 as u16
    }
}
