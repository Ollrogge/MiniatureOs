use anyhow::{anyhow, Context, Result};
use fatfs::FileAttributes;
use mbrman::BOOT_ACTIVE;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;
use walkdir::*;

// A Path is immutable. The owned version of Path is PathBuf

fn convert_elf_to_bin(filename: &Path) -> Result<()> {
    let bin_name = filename.with_extension("bin");
    let mut command = Command::new("objcopy");
    command
        .args(["-I", "elf64-x86-64"])
        .args(["-O", "binary"])
        .arg("--binary-architecture=i386:x86-64")
        .arg(filename)
        .arg(bin_name);

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("objcopy failed with exit code {}", status))
    }
}

fn build_mbr() -> Result<PathBuf> {
    let path = Path::new("x86_64/bios/mbr");
    let mut command = Command::new("cargo");
    println!("cargo:rerun-if-changed={}", path.display());
    command
        .arg("+nightly")
        .args(["install", "--path", path.to_str().unwrap()])
        .args([
            "--target",
            path.join("x86-mbr.json")
                .to_str()
                .context("Unable to construct target path")?,
        ])
        .args([
            "-Zbuild-std=core",
            "-Zbuild-std-features=compiler-builtins-mem",
        ])
        .args(["--profile", "mbr"]);

    let status = command.status()?;

    if !status.success() {
        return Err(anyhow!("failed to run install on mbr"));
    }

    let elf_file = Path::new("../target/x86-mbr/mbr/mbr");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin").canonicalize().unwrap())
}

fn build_stage2() -> Result<PathBuf> {
    let path = Path::new("x86_64/bios/stage2");
    let mut command = Command::new("cargo");
    println!("cargo:rerun-if-changed={}", path.display());
    command
        .arg("+nightly")
        .args(["install", "--path", path.to_str().unwrap()])
        .args([
            "--target",
            path.join("x86-stage2.json")
                .to_str()
                .context("Unable to construct target path")?,
        ])
        .args([
            "-Zbuild-std=core",
            "-Zbuild-std-features=compiler-builtins-mem",
        ])
        .args(["--profile", "stage2"]);

    let status = command.status()?;

    if !status.success() {
        return Err(anyhow!("failed to run install on mbr"));
    }

    let elf_file = Path::new("../target/x86-stage2/stage2/stage2");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin").canonicalize().unwrap())
}

fn build_stage3() -> Result<PathBuf> {
    let path: &Path = Path::new("x86_64/bios/stage3");
    let mut command = Command::new("cargo");
    println!("cargo:rerun-if-changed={}", path.display());
    command
        .arg("+nightly")
        .args(["install", "--path", path.to_str().unwrap()])
        .args([
            "--target",
            path.join("x86-stage3.json")
                .to_str()
                .context("Unable to construct target path")?,
        ])
        .args([
            "-Zbuild-std=core",
            "-Zbuild-std-features=compiler-builtins-mem",
        ])
        .args(["--profile", "stage3"]);

    let status = command.status()?;

    if !status.success() {
        return Err(anyhow!("failed to run install on mbr"));
    }

    let elf_file = Path::new("../target/x86-stage3/stage3/stage3");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin").canonicalize().unwrap())
}

fn build_stage4() -> Result<PathBuf> {
    let path: &Path = Path::new("x86_64/bios/stage4");
    let mut command = Command::new("cargo");
    println!("cargo:rerun-if-changed={}", path.display());
    command
        .arg("+nightly")
        .args(["install", "--path", path.to_str().unwrap()])
        .args([
            "--target",
            path.join("x86-stage4.json")
                .to_str()
                .context("Unable to construct target path")?,
        ])
        .args([
            "-Zbuild-std=core",
            "-Zbuild-std-features=compiler-builtins-mem",
        ])
        .args(["--profile", "stage4"]);

    let status = command.status()?;

    if !status.success() {
        return Err(anyhow!("failed to run install on mbr"));
    }

    let elf_file = Path::new("../target/x86-stage4/stage4/stage4");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin").canonicalize().unwrap())
}

pub fn build_bios() {
    println!("cargo:rerun-if-changed=../../x86_64");

    let mbr_path = build_mbr().unwrap();
    let stage2_path = build_stage2().unwrap();
    let stage3_path = build_stage3().unwrap();
    let stage4_path = build_stage4().unwrap();

    /*
    let src_dir = Path::new("../../x86_64/src");
    // Recursively add `cargo:rerun-if-changed` for all files in the directory
    if src_dir.exists() && src_dir.is_dir() {
        for entry in walkdir::WalkDir::new(src_dir) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            }
        }
    }
    */

    println!(
        "cargo:rustc-env=BIOS_BOOT_SECTOR_PATH={}",
        mbr_path.display()
    );
    println!(
        "cargo:rustc-env=BIOS_STAGE_2_PATH={}",
        stage2_path.display()
    );
    println!(
        "cargo:rustc-env=BIOS_STAGE_3_PATH={}",
        stage3_path.display()
    );
    println!(
        "cargo:rustc-env=BIOS_STAGE_4_PATH={}",
        stage4_path.display()
    );
}
