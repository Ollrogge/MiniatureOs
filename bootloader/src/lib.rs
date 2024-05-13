use anyhow::{anyhow, Context, Result};
use fatfs::FileAttributes;
use mbrman::BOOT_ACTIVE;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

const SECTOR_SIZE: u32 = 512;

struct DiskImageBuilder {
    kernel_path: PathBuf,
}

#[cfg(feature = "bios")]
pub mod bios;

impl DiskImageBuilder {
    pub fn new(kernel: &Path) -> Self {
        Self {
            kernel_path: PathBuf::from(kernel),
        }
    }

    #[cfg(feature = "bios")]
    pub fn create_bios_image(&self, out_path: &Path) {
        let bios_boot_sector_path = Path::new(env!("BIOS_BOOT_SECTOR_PATH"));
        let bios_stage_2_path = Path::new(env!("BIOS_STAGE_2_PATH"));
        let bios_stage_3_path = Path::new(env!("BIOS_STAGE_3_PATH"));
        let bios_stage_4_path = Path::new(env!("BIOS_STAGE_4_PATH"));

        self.create_mbr_disk(
            &bios_boot_sector_path,
            &bios_stage_2_path,
            &bios_stage_3_path,
            &bios_stage_4_path,
            out_path,
        )
        .unwrap();
    }

    #[cfg(feature = "bios")]
    fn create_mbr_disk(
        &self,
        mbr_path: &Path,
        second_stage_path: &Path,
        third_stage_path: &Path,
        fourth_stage_path: &Path,
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
            // no idea what this identifier describes.
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

        let fat_files = vec![
            ("stage3", third_stage_path),
            ("stage4", fourth_stage_path),
            ("kernel", &self.kernel_path),
        ];
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
}

#[cfg(feature = "bios")]
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
