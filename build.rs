use std::env;
use std::path::{Path, PathBuf};
fn main() {
    let bios_img = Path::new("bios.img");
    let kernel_path = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap());
    bootloader::bios::BiosBoot::new(&kernel_path).create_disk_image(&bios_img);

    println!("cargo:rustc-env=BIOS_PATH={}", bios_img.display());
    println!("cargo:rerun-if-changed=./x86_64/lib.rs");
}
