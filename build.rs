use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn main() {
    // TODO: this didn't work, therefore loop through dir and look at each
    // file individually
    //println!("cargo:rerun-if-changed=x86_64");
    let src_dir = Path::new("x86_64/src");
    // Recursively add `cargo:rerun-if-changed` for all files in the directory
    assert!(src_dir.exists() && src_dir.is_dir());
    for entry in walkdir::WalkDir::new(src_dir) {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }

    let bios_img = Path::new("bios.img");
    let kernel_path = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap());
    bootloader::bios::BiosBoot::new(&kernel_path).create_disk_image(&bios_img);

    // pass the disk image paths as env variables to the `main.rs`
    println!("cargo:rustc-env=BIOS_PATH={}", bios_img.display());

    for test_kernel in fs::read_dir("tests")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|e| {
            e.is_dir()
                && e.file_name()
                    .is_some_and(|f| f.to_str().unwrap().starts_with("test_kernel_"))
        })
        .map(|e| {
            e.file_name()
                .map(|f| f.to_str().unwrap().to_string())
                .unwrap()
        })
    {
        //panic!("Vars: {:?}", std::env::vars());
        let test_kernel_path = PathBuf::from(
            std::env::var_os(format!(
                "CARGO_BIN_FILE_{}_{}",
                test_kernel.to_uppercase(),
                test_kernel
            ))
            .unwrap(),
        );
        let path = format!("{}.img", test_kernel);
        let bios_img = Path::new(&path);
        bootloader::bios::BiosBoot::new(&test_kernel_path).create_disk_image(&bios_img);

        // path env variable for individual tests such that it can be run by test.rs
        println!(
            "cargo:rustc-env={}_BIOS_PATH={}",
            test_kernel.to_uppercase(),
            bios_img.display()
        );
    }
}
