use crate::cpm::dir_entry::CpmDirEntry;
use crate::cpm::file_id::FileId;
use crate::dsk::DskImage;
use crate::dsk::CHS;
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs::File;

/// CP/M filesystem parameters
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// sectors per track (CP/M format requires uniform formatting)
    pub sectors_per_track: u8,
    /// tracks (not cylinders!) at the beginning used for booting
    pub reserved_tracks: u8,
    /// size of a sector in bytes
    pub sector_size: u16,
    /// sectors per logical allocation block
    pub sectors_per_block: u8,
    /// number of blocks reserved for the file directory entries
    pub dir_blocks: u8,
}

pub enum LsMode {
    /// List all files (i.e. owned by all users), but not deleted files.
    All,
    /// List only files owned bya  given user.
    OwnedBy(u8),
    /// List all files, included deleted ones.
    Deleted,
}

/// Filesystem file list element.
#[derive(Clone, Debug)]
pub struct FileItem {
    /// User owning the file, or None for deleted items
    pub user: Option<u8>,
    /// File name with extension
    pub name: String,
    /// Size of the file
    pub size: usize,
    /// list of the blocks (LBAs) occupied by the file
    pub block_list: Vec<u16>,
}

pub struct CpmFs {
    params: Params,
    disk: DskImage,
    /// total number of filesystem blocks
    num_blocks: u16,
    /// raw directory entries (all, including unused ones)
    dir_entries: Vec<CpmDirEntry>,
    /// used logical blocks (LBA as index, true for used block)
    used_blocks: Vec<bool>,
}

impl CpmFs {
    pub fn load(f: &mut File, params: Params) -> Result<CpmFs> {
        // TODO: validate params ?

        let disk = DskImage::load(f)?;
        let dir_entries = Self::read_directory(&disk, &params)?;

        let num_blocks = (disk.num_cylinders() as u16 * disk.num_sides() as u16 * params.sectors_per_track as u16)
            / params.sectors_per_block as u16;
        let used_blocks = Self::calc_used_blocks(num_blocks, &dir_entries)?;

        Ok(CpmFs {
            params,
            disk,
            num_blocks,
            dir_entries,
            used_blocks,
        })
    }

    pub fn list_files(&self, mode: LsMode) -> Result<Vec<FileItem>> {
        let mut file_entries: HashMap<FileId, Vec<&CpmDirEntry>> = HashMap::new();
        let valid_block_range = self.params.dir_blocks as u16..self.num_blocks;

        let condition = |de: &&CpmDirEntry| match mode {
            LsMode::All => de.used(),
            LsMode::Deleted => de.used() || de.likely_deleted(&valid_block_range),
            LsMode::OwnedBy(num) => de.user == num,
        };

        // group all the extends belonging to each file
        for e in self.dir_entries.iter().filter(condition) {
            file_entries.entry(e.file_id()).or_insert_with(Vec::new).push(e);
        }

        // TODO: use map() ?
        let mut files: Vec<FileItem> = Vec::with_capacity(file_entries.len());
        for (k, v) in file_entries.iter() {
            let first = v[0];

            let mut sorted_extents = v.clone();
            sorted_extents.sort_unstable_by_key(|e| e.extent_number());
            let block_list = sorted_extents.iter().map(|e| e.blocks).flatten().collect();

            files.push(FileItem {
                user: first.owner(),
                name: first.file_name(),
                size: v.iter().map(|e| e.extent_size()).sum(),
                block_list,
            })
        }

        Ok(files)
    }

    /// Converts a logical sector index to a CHS sector address.
    fn lsi_to_chs(params: &Params, sides: u8, lsi: u16) -> CHS {
        let track = lsi / params.sectors_per_track as u16 + params.reserved_tracks as u16;
        // note: +1, because sector IDs start from 1
        let sector = (lsi % params.sectors_per_track as u16) as u8 + 1;

        let cylinder = (track / sides as u16) as u8;
        let head = (track % sides as u16) as u8;
        CHS { cylinder, head, sector }
    }

    fn read_directory(disk: &DskImage, params: &Params) -> Result<Vec<CpmDirEntry>> {
        let num_sectors = params.dir_blocks as u16 * params.sectors_per_block as u16;
        let total_slots = num_sectors * params.sector_size / 32;
        let mut entries = Vec::with_capacity(total_slots as usize);

        let sides = disk.num_sides();
        // note: it starts from logical sector 0
        for lsi in 0..num_sectors {
            let sector = disk.sector_as_slice(Self::lsi_to_chs(params, sides, lsi))?;

            let sector_entries: Vec<CpmDirEntry> = sector
                .chunks(32)
                .map(|chunk| CpmDirEntry::from_bytes(chunk))
                .collect::<Result<Vec<_>>>()?;
            entries.extend(sector_entries);
        }
        Ok(entries)
    }

    fn calc_used_blocks(num_blocks: u16, dir_entries: &Vec<CpmDirEntry>) -> Result<Vec<bool>> {
        let mut used_blocks = vec![false; num_blocks as usize];
        for e in dir_entries.iter().filter(|e| e.used()) {
            for b in e.blocks {
                if b != 0 {
                    if used_blocks[b as usize] {
                        bail!("Block {} used more than once", b)
                    }
                    used_blocks[b as usize] = true;
                }
            }
        }
        Ok(used_blocks)
    }
}

#[cfg(test)]
mod tests {
    use crate::cpm::cpm_fs::LsMode::All;
    use crate::cpm::cpm_fs::{CpmFs, Params};
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    fn test_load_save_dsk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/03.dsk");
        let mut file = File::open(path).unwrap();

        let params = Params {
            sectors_per_track: 9,
            reserved_tracks: 2,
            sector_size: 512,
            sectors_per_block: 4,
            dir_blocks: 4,
        };
        let fs = CpmFs::load(&mut file, params).unwrap();
        let files = fs.list_files(All).unwrap();
        dbg!(&files);
    }
}
