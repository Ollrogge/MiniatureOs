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

// A Path is immutable. The owned version of Path is PathBuf

const SECTOR_SIZE: u32 = 512;

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
    let path = Path::new("Bootloader/x86_64/mbr");
    let mut command = Command::new("cargo");
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

    let elf_file = Path::new("target/x86-mbr/mbr/mbr");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin"))
}

fn build_stage2() -> Result<PathBuf> {
    let path = Path::new("Bootloader/x86_64/stage2");
    let mut command = Command::new("cargo");
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

    let elf_file = Path::new("target/x86-stage2/stage2/stage2");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin"))
}

fn build_stage3() -> Result<PathBuf> {
    let path: &Path = Path::new("Bootloader/x86_64/stage3");
    let mut command = Command::new("cargo");
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

    let elf_file = Path::new("target/x86-stage3/stage3/stage3");
    convert_elf_to_bin(&elf_file)?;

    Ok(elf_file.with_extension("bin"))
}

fn create_fat_filesystem(files: Vec<(&str, &Path)>, out_path: &Path) -> Result<()> {
    let mut fat_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(out_path)
        .context("Failed to open tmp file")?;

    let mut needed_size = 0x0;
    for (_, path) in files.iter() {
        needed_size += fs::metadata(path).context("Failed to get metadata")?.len();
    }
    const MB: u64 = 1024 * 1024;
    let fat_size_padded_and_rounded = ((needed_size + 1024 * 64 - 1) / MB + 1) * MB + MB;

    fat_file
        .set_len(fat_size_padded_and_rounded)
        .context("Failed to set fat file length")?;

    // FAT type is determined based on total number of clusters
    let format_options = fatfs::FormatVolumeOptions::new().volume_label(*b"MiniatureOs");
    fatfs::format_volume(&fat_file, format_options).context("Failed tor format volume")?;
    let fs = fatfs::FileSystem::new(&mut fat_file, fatfs::FsOptions::new())
        .context("fatfs::Filesystem new")?;

    let root_dir = fs.root_dir();

    for (name, path) in files.iter() {
        let mut src_file = fs::File::open(path).context("Failed to open stage file")?;
        let mut dest_file = root_dir
            .create_file(name)
            .context("Failed to create file in FAT root")?;

        dest_file.truncate()?;

        io::copy(&mut src_file, &mut dest_file).context("Failed to copy file contents")?;
    }

    Ok(())
}

fn create_mbr_disk(
    mbr_path: &Path,
    second_stage_path: &Path,
    third_stage_path: &Path,
    out_path: &Path,
) -> Result<()> {
    let mut mbr_file = File::open(&mbr_path).context("Failed to open mbr bin file")?;

    let mut mbr =
        mbrman::MBR::read_from(&mut mbr_file, SECTOR_SIZE).context("Failed to read mbr")?;

    let mut second_stage =
        File::open(&second_stage_path).context("Failed to open second stage file")?;

    let second_stage_len = second_stage
        .metadata()
        .context("Unable to obtain second stage file size")?
        .len();

    let second_stage_start_sector = 1;
    let second_stage_sectors =
        ((second_stage_len + (SECTOR_SIZE - 1) as u64) / SECTOR_SIZE as u64) as u32;

    mbr[1] = mbrman::MBRPartitionEntry {
        boot: mbrman::BOOT_ACTIVE,
        starting_lba: second_stage_start_sector,
        // make sure we round up
        sectors: second_stage_sectors,
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
        u64::from(second_stage_start_sector * SECTOR_SIZE)
    );
    io::copy(&mut second_stage, &mut disk)
        .context("failed to copy second stage binary to MBR disk image")?;

    let fat_files = vec![("stage3", third_stage_path)];
    let mut boot_partition = NamedTempFile::new().context("Unable to create temp file")?;
    create_fat_filesystem(fat_files, boot_partition.path())?;

    let boot_partition_len = boot_partition
        .as_file()
        .metadata()
        .context("Unable to get tmp file metadata")?
        .len();
    let boot_partition_start_sector = second_stage_start_sector + second_stage_sectors;
    let boot_partition_sectors =
        ((boot_partition_len + (SECTOR_SIZE - 1) as u64) / SECTOR_SIZE as u64) as u32;

    mbr[2] = mbrman::MBRPartitionEntry {
        boot: mbrman::BOOT_ACTIVE,
        starting_lba: boot_partition_start_sector,
        sectors: boot_partition_sectors,
        // FAT32 with LBA
        sys: 0xc,
        first_chs: mbrman::CHS::empty(),
        last_chs: mbrman::CHS::empty(),
    };

    mbr.write_into(&mut disk)
        .context("Writing boot parition info to mbr failed")?;

    disk.seek(SeekFrom::Start(
        (boot_partition_start_sector * SECTOR_SIZE).into(),
    ))
    .context("seek failed")?;

    io::copy(&mut boot_partition, &mut disk)
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
    let stage3_path = build_stage3().unwrap();
    let disk_path = Path::new("disk_image.img");

    create_mbr_disk(&mbr_path, &stage2_path, &stage3_path, &disk_path).unwrap();

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
