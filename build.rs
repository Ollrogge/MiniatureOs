use anyhow::{anyhow, Context, Result};
use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
};

// A Path is immutable. The owned version of Path is PathBuf

const SECTOR_SIZE: u32 = 512;

fn convert_elf_to_bin(filename: &Path) -> Result<()> {
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

fn build_mbr() -> Result<PathBuf> {
    let path = Path::new("Bootloader/x86_64/mbr");
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

    let elf_file = Path::new("target/x86-mbr/mbr/mbr");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin"))
}

fn build_stage2() -> Result<PathBuf> {
    let path = Path::new("Bootloader/x86_64/stage2");
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

    let elf_file = Path::new("target/x86-stage2/stage2/stage2");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin"))
}

fn create_mbr_disk(mbr_path: &Path, second_stage_path: &Path, out_path: &Path) -> Result<()> {
    let mut mbr_file = File::open(&mbr_path).context("Failed to open mbr bin file")?;

    let mut mbr =
        mbrman::MBR::read_from(&mut mbr_file, SECTOR_SIZE).context("Failed to read mbr")?;

    let mut second_stage =
        File::open(&second_stage_path).context("Failed to open second stage file")?;

    let second_stage_len = second_stage
        .metadata()
        .and_then(|m| Ok(m.len()))
        .context("Unable to obtain file size")?;

    mbr[1] = mbrman::MBRPartitionEntry {
        boot: mbrman::BOOT_ACTIVE,
        starting_lba: 1,
        // make sure we round up
        sectors: ((second_stage_len + (SECTOR_SIZE - 1) as u64) / SECTOR_SIZE as u64) as u32,
        // no idea
        sys: 0x20,
        first_chs: mbrman::CHS::empty(),
        last_chs: mbrman::CHS::empty(),
    };

    let mut disk = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(out_path)
        .context("Failed to create MBR disk")?;

    mbr.write_into(&mut disk)
        .context("Writing to mbr disk failed")?;

    assert_eq!(
        disk.stream_position()
            .context("failed to get disk image seek position")?,
        u64::from(SECTOR_SIZE)
    );
    io::copy(&mut second_stage, &mut disk)
        .context("failed to copy second stage binary to MBR disk image")?;

    Ok(())
}

fn main() {
    bios_main();
}

fn bios_main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    eprintln!("Out dir: {:?}", out_dir);

    let mbr_path = build_mbr().unwrap();
    let stage2_path = build_stage2().unwrap();
    let disk_path = Path::new("disk_image.img");

    create_mbr_disk(&mbr_path, &stage2_path, &disk_path).unwrap();

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
