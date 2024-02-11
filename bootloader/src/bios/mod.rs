use crate::DiskImageBuilder;
use std::path::Path;

pub struct BiosBoot {
    builder: DiskImageBuilder,
}

impl BiosBoot {
    pub fn new(kernel: &Path) -> Self {
        Self {
            builder: DiskImageBuilder::new(kernel),
        }
    }

    pub fn create_disk_image(&self, out_path: &Path) {
        self.builder.create_bios_image(out_path)
    }
}
