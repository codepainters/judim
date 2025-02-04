use super::structs::{DskFileHeader, TrackInfo};
use anyhow::{anyhow, bail, Result};
use binrw::{BinReaderExt, BinWrite};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

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

impl DskImage {
    pub fn load(f: &mut File) -> Result<Self> {
        let header: DskFileHeader = f.read_le()?;
        let mut tracks = Vec::with_capacity((header.num_cylinders * header.num_sides) as usize);

        for c in 0..header.num_cylinders {
            for h in 0..header.num_sides {
                let idx = c * header.num_sides + h;

                let file_pos = f.seek(SeekFrom::Current(0))?;
                let track: DskImageTrack = DskImageTrack::load(f)?;
                let loaded_bytes = f.seek(SeekFrom::Current(0))? - file_pos;
                if loaded_bytes != 256 * header.track_sizes[idx as usize] as u64 {
                    bail!("Track {} size invalid", idx);
                }

                if track.header.cylinder_number != c || track.header.side_number != h {
                    bail!("Invalid track order");
                }

                tracks.push(track);
            }
        }

        Ok(Self { header, tracks })
    }

    pub fn save(&self, f: &mut File) -> Result<()> {
        f.seek(SeekFrom::Start(0))?;
        self.header.write_le(f)?;
        for track in &self.tracks {
            track.save(f)?;
        }
        Ok(())
    }

    pub fn num_cylinders(&self) -> u8 {
        self.header.num_cylinders
    }

    pub fn num_sides(&self) -> u8 {
        self.header.num_sides
    }

    fn ch_to_track_index(&self, cylinder: u8, head: u8) -> Result<usize> {
        if head >= self.header.num_sides {
            bail!("Invalid head (side) number: {}", head);
        }
        if cylinder >= self.header.num_cylinders {
            bail!("Invalid cylinder number: {}", cylinder);
        }

        Ok((cylinder * self.header.num_sides + head) as usize)
    }

    pub fn sector_as_slice(&self, chs: CHS) -> Result<&[u8]> {
        let track = self.ch_to_track_index(chs.cylinder, chs.head)?;
        self.tracks[track]
            .sector_as_slice(chs.sector)
            .ok_or(anyhow!("Sector not found"))
    }

    pub fn sector_as_slice_mut(&mut self, chs: CHS) -> Result<&mut [u8]> {
        let track = self.ch_to_track_index(chs.cylinder, chs.head)?;
        self.tracks[track]
            .sector_as_slice_mut(chs.sector)
            .ok_or(anyhow!("Sector not found"))
    }
}

struct DskImageTrack {
    header: TrackInfo,
    /// data of all track sectors, as stored in the image
    sector_data: Vec<u8>,
    /// maps sector ID (R in uPD765 parlance) to sector index in the track image
    sector_index: [Option<usize>; 256],
}

impl DskImageTrack {
    fn load(f: &mut File) -> Result<Self> {
        let header: TrackInfo = f.read_le()?;

        let mut sector_index = [None; 256];
        for (idx, s) in header.sectors.iter().enumerate() {
            if s.sector_size != header.sector_size {
                bail!("Variable sector size not supported");
            }

            if let Some(_) = sector_index[s.sector_id as usize] {
                bail!(
                    "sector ID {} on the track c={}, h={} is not unique",
                    s.cylinder,
                    header.cylinder_number,
                    header.side_number
                );
            }
            sector_index[s.sector_id as usize] = Some(idx);
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
        self.sector_index[sector_id as usize]
            .map(|i| &self.sector_data[i as usize * sector_size..(i + 1) as usize * sector_size])
    }

    fn sector_as_slice_mut(&mut self, sector_id: u8) -> Option<&mut [u8]> {
        let sector_size = self.header.sector_size as usize;
        self.sector_index[sector_id as usize]
            .map(|i| &mut self.sector_data[i as usize * sector_size..(i + 1) as usize * sector_size])
    }
}
#[cfg(test)]
mod tests {
    use crate::dsk::image::DskImage;
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    fn test_load_save_dsk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/03.dsk");
        let mut file = File::open(path).unwrap();

        let image = DskImage::load(&mut file).unwrap();

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/out.dsk");
        let mut file = File::create(path).unwrap();
        image.save(&mut file).unwrap();
    }
}
