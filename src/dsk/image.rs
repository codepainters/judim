use super::structs::{DskFileHeader, TrackInfo};
use anyhow::{anyhow, bail, Result};
use binrw::{BinReaderExt, BinWrite};
use std::fs::File;
use std::io::{Read, Seek, Write};

/// CHS encapsulates cylinder/head/sector address
pub struct CHS {
    /// cylinder number, 0 based
    pub cylinder: u8,
    /// head (side) number, 0 or 1
    pub head: u8,
    /// sector id
    pub sector: u8,
}

pub struct DskImage {
    header: DskFileHeader,
    tracks: Vec<DskImageTrack>,
}

struct DskImageTrack {
    header: TrackInfo,
    /// data of all track sectors, as stored in the image
    sector_data: Vec<u8>,
    /// maps sector ID (R in uPD765 parlance) to sector index in the track image
    sector_index: [Option<u8>; 256],
}

// TODO:
//   - in cpm_fs.py we do self._image.tracks[c * 2 + h], bad
//     - code validating (during loading), that tracks are indeed in this order

impl DskImage {
    pub fn load(f: &mut File) -> Result<Self> {
        let header: DskFileHeader = f.read_le()?;
        let mut tracks = Vec::with_capacity((header.num_cylinders * header.num_sides) as usize);

        for _ in 0..(header.num_cylinders * header.num_sides) {
            let track: DskImageTrack = DskImageTrack::load(f)?;

            /// TODO: check, if file offset is as expected
            /// TODO: check track ordering
            tracks.push(track);
        }

        Ok(Self { header, tracks })
    }

    fn ch_to_track_index(&self, cylinder: u8, head: u8) -> usize {
        // TODO: validate cylinder/head range
        (cylinder * 2 + head) as usize
    }

    fn sector_as_slice(&self, chs: CHS) -> Result<&[u8]> {
        let track = self.ch_to_track_index(chs.cylinder, chs.head);
        self.tracks[track]
            .sector_as_slice(chs.sector)
            .ok_or(anyhow!("Sector not found"))
    }

    fn sector_as_slice_mut(&mut self, chs: CHS) -> Result<&mut [u8]> {
        let track = self.ch_to_track_index(chs.cylinder, chs.head);
        self.tracks[track]
            .sector_as_slice_mut(chs.sector)
            .ok_or(anyhow!("Sector not found"))
    }
}

impl DskImageTrack {
    fn load(f: &mut File) -> Result<Self> {
        let header: TrackInfo = f.read_le()?;

        let mut sector_index = [None; 256];
        for s in &header.sectors {
            if s.sector_size != header.sector_size {
                bail!("Variable sector size not supported");
            }

            if let Some(_) = sector_index[s.cylinder as usize] {
                bail!("sector IDs on the track are not unique");
            }
            sector_index[s.sector_id as usize] = Some(s.sector_id);
        }

        let buffer_size = header.sector_size as usize * header.num_sectors as usize;
        let mut sector_data = vec![0; buffer_size];
        f.read_exact(sector_data.as_mut_slice())?;

        Ok(DskImageTrack {
            header,
            sector_data,
            sector_index,
        })
    }

    fn save(&self, f: &mut File) -> Result<()> {
        self.header.write_le(f)?;
        f.write_all(&self.sector_data)?;
        Ok(())
    }

    fn sector_as_slice(&self, sector_id: u8) -> Option<&[u8]> {
        let sector_size = self.header.sector_size as usize;
        self.sector_index[sector_id].map(|i| &self.sector_data[i * sector_size..(i + 1) * sector_size])
    }

    fn sector_as_slice_mut(&mut self, sector_id: u8) -> Option<&mut [u8]> {
        let sector_size = self.header.sector_size as usize;
        self.sector_index[sector_id].map(|i| &self.sector_data[i * sector_size..(i + 1) * sector_size])
    }
}
#[cfg(test)]
mod tests {
    use crate::dsk::image::DskImage;
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    fn test_load_dsk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/03.dsk");
        let mut file = File::open(path).unwrap();

        let image = DskImage::load(&mut file).unwrap();
    }
}
