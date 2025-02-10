use anyhow::{bail, Result};
use binrw::{binrw, BinReaderExt};
use std::io::Cursor;
use std::ops::{Range, Shl};

// TODO:
//   - validate block list
//   - implement saving back to slice
//   - parse extra flags (get rid of binrw? use calc?)
//   - encapsulate

/// CpmDirEntry structure represents a directory entry as stored
/// in the CP/M filesystem directory.
///
/// Note: depending on the size of the filesystem, DirEntry
/// can store either 16 * u8 or 8 * u16 block numbers. This
/// implementation hardcodes the second case.
#[binrw]
#[brw(little)]
pub struct CpmDirEntry {
    /// user ID (0..15) or 0xE5 for deleted entries
    pub user: u8,
    /// file name, 0x20-padded
    pub name: [u8; 8],
    /// extension, 0x20-padded
    pub extension: [u8; 3],
    /// extent number, used for files spanning more than one dir entry
    pub extent_l: u8,
    _reserved: u8,
    /// extent number, higher byte
    pub extent_h: u8,
    /// file size expressed as number of 128-byte records
    pub record_count: u8,
    /// block numbers
    pub blocks: [u16; 8],
}

#[derive(Eq, PartialEq, Hash)]
pub struct FileId {
    user: u8,
    name: [u8; 8],
    extension: [u8; 3],
}

impl CpmDirEntry {
    pub fn from_bytes(data: &[u8]) -> Result<CpmDirEntry> {
        let d: CpmDirEntry = Cursor::new(data).read_le()?;
        if d.user != 0xE5 && d.user > 15 {
            bail!("Invalid user number: {}", d.user);
        }
        if d.user != 0xE5 && !(d.name.is_ascii() && d.extension.is_ascii()) {
            bail!("Non-ASCII name or extension");
        }

        // TODO: validate, that there are no non-zero block entries after first zero

        Ok(d)
    }

    pub fn extent_size(&self) -> usize {
        self.record_count as usize * 128
    }

    pub fn extent_number(&self) -> u16 {
        (self.extent_h as u16).shl(8) | self.extent_l as u16
    }

    pub fn file_name(&self) -> String {
        let name = String::from_utf8_lossy(&self.name);
        let extension = String::from_utf8_lossy(&self.extension);
        format!("{}.{}", name.trim_end(), extension.trim_end())
    }
    //
    // pub fn deleted(&self) -> bool {
    //     self.user == 0xE5
    // }

    pub fn used(&self) -> bool {
        self.user != 0xE5
    }

    #[allow(unused)]
    pub fn likely_deleted(&self, valid_block_range: Range<u16>) -> bool {
        false
    }

    pub fn file_id(&self) -> FileId {
        FileId {
            user: self.user,
            name: self.name,
            extension: self.extension,
        }
    }
}
