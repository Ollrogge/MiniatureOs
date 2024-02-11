use crate::util::UnwrapOrFail;
/// An entry in a partition table.
///
/// Based on https://docs.rs/mbr-nostd
///
/// Don't need all entries therefore this is not an exact replica
#[derive(Copy, Clone, Eq, PartialEq)]
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

pub fn get_partition(partition_table: &[u8], index: usize) -> PartitionTableEntry {
    const ENTRY_SIZE: usize = 16;

    let offset = index * ENTRY_SIZE;
    let buffer = partition_table.get(offset..).unwrap_or_fail(b'c');

    let bootable_raw = *buffer.first().unwrap_or_fail(b'd');
    let bootable = bootable_raw == 0x80;

    let partition_type = *buffer.get(4).unwrap_or_fail(b'e');

    let lba = u32::from_le_bytes(
        buffer
            .get(8..)
            .and_then(|s| s.get(..4))
            .and_then(|s| s.try_into().ok())
            .unwrap_or_fail(b'e'),
    );
    let len = u32::from_le_bytes(
        buffer
            .get(12..)
            .and_then(|s| s.get(..4))
            .and_then(|s| s.try_into().ok())
            .unwrap_or_fail(b'f'),
    );
    PartitionTableEntry::new(bootable, partition_type, lba, len)
}
