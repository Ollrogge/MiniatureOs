use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

// A Path is immutable. The owned version of Path is PathBuf

fn convert_elf_to_bin(filename: &Path) -> Result<()> {
    eprintln!("Path: {:?}", filename);
    let bin_name = filename.with_extension("bin");
    let mut command = Command::new("objcopy");
    command
        .args(["-I", "elf32-i386"])
        .args(["-O", "binary"])
        .arg(filename)
        .arg(bin_name);

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("objcopy failed with exit code {}", status))
    }
}

fn build_mbr() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Bootloader")
        .join("x86_64")
        .join("mbr");
    let mut command = Command::new("cargo");
    command
        .arg("+nightly")
        .args(["install", "--path", "Bootloader/x86_64/mbr"])
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

    let elf_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/x86-mbr/mbr/mbr");

    convert_elf_to_bin(&elf_file)?;

    Ok(())
}

fn build_stage2() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Bootloader")
        .join("x86_64")
        .join("stage2");
    let mut command = Command::new("cargo");
    command
        .arg("+nightly")
        .args(["install", "--path", "Bootloader/x86_64/stage2"])
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

    let elf_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/x86-stage2/stage2/stage2");

    convert_elf_to_bin(&elf_file)?;

    Ok(())
}

fn main() {
    bios_main();
}

fn bios_main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    eprintln!("Out dir: {:?}", out_dir);

    build_mbr().unwrap();
    build_stage2().unwrap();

    // Run the bios build commands concurrently.
    // (Cargo already uses multiple threads for building dependencies, but these
    // BIOS crates don't have enough dependencies to utilize all cores on modern
    // CPUs. So by running the build commands in parallel, we increase the number
    // of utilized cores.)
    /*
    let (bios_boot_sector_path, bios_stage_2_path, bios_stage_3_path, bios_stage_4_path) = (
        build_bios_boot_sector(&out_dir),
        build_bios_stage_2(&out_dir),
        build_bios_stage_3(&out_dir),
        build_bios_stage_4(&out_dir),
    )
        .join()
        .await;
    */
}
