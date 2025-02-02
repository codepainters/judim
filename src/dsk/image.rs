use super::structs::{DskFileHeader, TrackInfo};
use anyhow::Result;
use binrw::BinReaderExt;
use std::fs::File;
use std::io::{Seek, SeekFrom};

pub struct DskImage {
    header: DskFileHeader,
    tracks: Vec<TrackInfo>,
}

// TODO:
//   - add CHS location type
//   - add struct holding TrackInfo and actual sector data
//   - replace DiskImage.tacks vec type with this new struct
//   - in cpm_fs.py we do self._image.tracks[c * 2 + h], bad
//     - function mapping CH part to track index
//     - code validating (during loading), that tracks are indeed in this order
//     - function giving CHS-addressed sector as a slice (mutable/immutable?)

impl DskImage {
    pub fn load(f: &mut File) -> Result<Self> {
        let header: DskFileHeader = f.read_le()?;
        let mut tracks = Vec::with_capacity((header.num_cylinders * header.num_sides) as usize);

        // temporary
        for _ in 0..(header.num_cylinders * header.num_sides) {
            let track: TrackInfo = f.read_le()?;
            f.seek(SeekFrom::Current(track.num_sectors as i64 * track.sector_size as i64))?;
            tracks.push(track);
        }

        Ok(Self { header, tracks })
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
