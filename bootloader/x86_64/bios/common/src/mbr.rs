//! This module implements the handling of the PartitionTable stored at the end
//! of the master boot record

/// An entry in a partition table.
///
/// Based on https://docs.rs/mbr-nostd
///
/// Don't need all entries therefore this is not an exact replica
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct PartitionTableEntry {
    /// Whether this partition is a boot partition.
    pub bootable: bool,

    /// The type of partition in this entry.
    pub partition_type: u8,

    /// The index of the first block of this entry.
    pub logical_block_address: u32,

    /// The total number of blocks in this entry.
    pub sector_count: u32,
}

pub const PARTITION_TABLE_ENTRY_COUNT: usize = 0x4;
pub const PARTITION_TABLE_ENTRY_SIZE: usize = 0x10;

impl PartitionTableEntry {
    pub fn new(
        bootable: bool,
        partition_type: u8,
        logical_block_address: u32,
        sector_count: u32,
    ) -> PartitionTableEntry {
        PartitionTableEntry {
            bootable,
            partition_type,
            logical_block_address,
            sector_count,
        }
    }
}

pub fn get_partition_table_entry(partition_table: &[u8], index: usize) -> PartitionTableEntry {
    const ENTRY_SIZE: usize = 16;

    let offset = index * ENTRY_SIZE;
    let buffer = partition_table.get(offset..).unwrap();

    let bootable_raw = *buffer.first().unwrap();
    let bootable = bootable_raw == 0x80;

    let partition_type = *buffer.get(4).unwrap();

    let lba = u32::from_le_bytes(
        buffer
            .get(8..)
            .and_then(|s| s.get(..4))
            .and_then(|s| s.try_into().ok())
            .unwrap(),
    );
    let len = u32::from_le_bytes(
        buffer
            .get(12..)
            .and_then(|s| s.get(..4))
            .and_then(|s| s.try_into().ok())
            .unwrap(),
    );
    PartitionTableEntry::new(bootable, partition_type, lba, len)
}
