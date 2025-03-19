use crate::cpm::file_id::FileId;
use anyhow::{bail, Result};
use std::ops::Range;

pub const BLOCKS_PER_EXTENT: usize = 8;

/// CpmDirEntry structure represents a directory entry as stored
/// in the CP/M filesystem directory.
///
/// Note: depending on the size of the filesystem, DirEntry
/// can store either 16 * u8 or 8 * u16 block numbers. This
/// implementation hardcodes the second case.
pub struct CpmDirEntry {
    pub file_id: FileId,
    /// extent number, used for files spanning more than one dir entry
    pub extent: u16,
    /// file size expressed as number of 128-byte records
    pub record_count: u8,
    /// block numbers
    blocks: [u16; BLOCKS_PER_EXTENT],
    /// read-only flag
    pub read_only: bool,
    /// system file flag
    pub system_file: bool,
    /// archived file flag
    pub archived: bool,
}

impl CpmDirEntry {
    pub fn from_bytes(data: &[u8; 32]) -> Result<CpmDirEntry> {
        let file_id_bytes = &data[0..12].try_into().unwrap();
        let file_id = FileId::from_bytes(file_id_bytes)?;

        let (x_h, x_l) = (data[14] as u16, data[12] as u16);
        let extent = (x_h << 8) + x_l;
        let record_count = data[15];

        let block_bytes = &data[16..32];
        let mut blocks = [0u16; BLOCKS_PER_EXTENT];
        for (i, chunk) in block_bytes.chunks_exact(2).enumerate() {
            blocks[i] = u16::from_le_bytes([chunk[0], chunk[1]]);
        }

        // Note: only check validity for actually used entries! Still we want
        // to keep the info for unsued (possibly deleted) entries.
        if file_id.user != 0xE5 {
            if !Self::has_only_trailing_zeros(&blocks) {
                bail!(
                    "Invalid block list for {} extent {}: {:?}",
                    file_id.filename(),
                    extent,
                    blocks
                );
            }
        }

        let read_only = file_id.extension[0] & 0x80 != 0;
        let system_file = file_id.extension[1] & 0x80 != 0;
        let archived = file_id.extension[2] & 0x80 != 0;

        Ok(CpmDirEntry {
            file_id,
            extent,
            record_count,
            blocks,
            read_only,
            system_file,
            archived,
        })
    }

    pub fn new(file_id: FileId, extent: u16, record_count: u8, blocks: &[u16]) -> CpmDirEntry {
        assert!(blocks.len() <= BLOCKS_PER_EXTENT);
        let mut blocks_array = [0u16; BLOCKS_PER_EXTENT];
        blocks_array[0..blocks.len()].copy_from_slice(blocks);

        CpmDirEntry {
            file_id,
            extent,
            record_count,
            blocks: blocks_array,
            read_only: false,
            system_file: false,
            archived: false,
        }
    }

    fn has_only_trailing_zeros(s: &[u16]) -> bool {
        match s.iter().position(|&x| x == 0) {
            Some(pos) => s[pos..].iter().all(|&x| x == 0),
            None => true, // No zeros at all
        }
    }

    pub fn extent_size(&self) -> usize {
        self.record_count as usize * 128
    }

    pub fn file_name(&self) -> String {
        self.file_id.filename()
    }

    pub fn used(&self) -> bool {
        self.file_id.user != 0xE5
    }

    pub fn owner(&self) -> Option<u8> {
        if self.used() {
            Some(self.file_id.user)
        } else {
            None
        }
    }

    pub fn likely_deleted(&self, valid_block_range: &Range<u16>) -> bool {
        // heuristic: marked as unused, but valid block list. This eliminates entries
        // filled with 0xE5 after formatting.
        self.file_id.user == 0xE5 && self.blocks.iter().all(|b| *b == 0 || valid_block_range.contains(b))
    }

    /// Returns list of actual blocks used by this entry (i.e. trailing zeros get trimmed).
    pub fn blocks(&self) -> Vec<u16> {
        if let Some(last_nonzero) = self.blocks.iter().rposition(|&x| x != 0) {
            self.blocks[0..last_nonzero + 1].to_vec()
        } else {
            vec![]
        }
    }
}
