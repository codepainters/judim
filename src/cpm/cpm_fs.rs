use crate::cpm::dir_entry::CpmDirEntry;
use crate::dsk::DskImage;
use crate::dsk::CHS;
use anyhow::{bail, Result};
use std::fs::File;

/// CP/M filesystem parameters
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// sectors per track (CP/M format requires uniform formatting)
    sectors_per_track: u8,
    /// tracks (not cylinders!) at the beginning used for booting
    reserved_tracks: u8,
    /// size of a sector in bytes
    sector_size: u16,
    /// sectors per logical allocation block
    sectors_per_block: u8,
    /// number of blocks reserved for the file directory entries
    dir_blocks: u8,
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

            // TODO: use vec/map/collect here
            for entry_bytes in sector.chunks(32) {
                entries.push(CpmDirEntry::from_bytes(entry_bytes)?)
            }
        }
        Ok(entries)
    }

    fn calc_used_blocks(num_blocks: u16, dir_entries: &Vec<CpmDirEntry>) -> Result<Vec<bool>> {
        let mut used_blocks = vec![false; num_blocks as usize];
        for e in dir_entries.iter().filter(|e| !e.deleted()) {
            dbg!(e.file_name());
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
    use std::fs::File;
    use std::path::PathBuf;
    use crate::cpm::cpm_fs::{CpmFs, Params};

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
    }
}
