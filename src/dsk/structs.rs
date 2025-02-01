use binrw::binrw;

// This module defines all the structures defined in the DSK
// image file format, with serialization/deserialization code.
//
// DSK image description can be found here:
// - https://cpctech.cpc-live.com/docs/extdsk.html
// - https://sinclair.wiki.zxnet.co.uk/wiki/DSK_format

#[binrw]
#[brw(little)]
#[br(magic = b"EXTENDED CPC DSK File\r\nDisk-Info\r\n")]
pub struct DskFileHeader {
    /// Name of the program that created the file, ASCII, zero-padded
    name_of_creator: [u8; 14],
    /// Number fo the disk's cylinders
    num_cylinders: u8,
    /// Number fo the disk's sides
    num_sides: u8,
    _unused: [u8; 2],
    /// Sizes of consecutive track info blocks in 256 bytes units.
    /// Note: I don't know how to convert [u8] to [u16] with binrw.
    #[br(count = num_cylinders * num_sides, align_after=256)]
    track_sizes: Vec<u8>,
}

#[derive(Debug)]
#[binrw]
#[brw(little)]
#[brw(magic = b"Track-Info\r\n")]
pub struct TrackInfo {
    /// Cylinder number, 0-based
    #[brw(pad_before = 4)]
    cylinder_number: u8,
    /// Side number, 0 or 1
    side_number: u8,

    _unused1: [u8; 2],

    /// Size of the sector (stored as u8 with unit of 256 bytes)
    #[br(map = |x: u8| x as u16 * 256)]
    #[bw(map = |x| (x / 256) as u8)]
    sector_size: u16,

    /// Number of sectors on this particular track (tracks may vary)
    num_sectors: u8,
    /// GAP#3 length, as defined in uPD765 datasheet
    gap3_length: u8,

    _unused2: u8,

    /// Metadata of actual sectors
    #[br(count = num_sectors, align_after=256)]
    #[bw(align_after = 256)]
    sectors: Vec<SectorInfo>,
}

/// SectorInfo contains metadata for a single sector within a track.
#[derive(Debug)]
#[binrw]
#[brw(little)]
pub struct SectorInfo {
    /// Cylinder number, equivalent to C parameter in uPD765 commands
    cylinder: u8,
    /// Side number, equivalent to S parameter in uPD765 commands
    side: u8,
    /// Sector ID, equivalent to R parameter in uPD765 commands
    sector_id: u8,

    /// Sector size, equivalent to N parameter in uPD765 commands
    #[br(map = |x: u8| x as u16 * 256)]
    #[bw(map = |x| (x / 256) as u8)]
    sector_size: u16,

    /// uPD765 Status Register 1 value
    fdc_st1: u8,
    /// uPD765 Status Register 2 value
    fdc_st2: u8,

    /// Actual length of the sector data.
    ///
    /// One description says:
    ///     If a sector has weak/random data, there are multiple copies stored.
    ///     This field stores the size of all the copies.
    ///
    /// Another interpretation:
    ///     Used for some forms of copy protection where the written sector
    ///     is smaller than the requested sector. The sector still has sector_size bytes reserved
    ///     in the file, but any emulator should only return actual_data_length  bytes before
    ///     reporting an FDC error to the disk (Preferably the ones stored in fdc_st1/st2).
    actual_data_length: u16
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::PathBuf;
    use binrw::{io::Cursor, BinReaderExt, BinWrite};
    use super::{DskFileHeader, SectorInfo, TrackInfo};

    fn load_test_data(offset: u64, length: u64) -> std::io::Result<Vec<u8>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/03.dsk");
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;

        let mut buf = vec![0; length as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    #[test]
    fn test_sector_info_serialization() {
        let sector_info = SectorInfo {
            cylinder: 2,
            side: 1,
            sector_id: 5,
            sector_size: 1536, // 1536 = 6 * 256
            fdc_st1: 17,
            fdc_st2: 18,
            actual_data_length: 512,
        };

        // Serialize SectorInfo to a Vec<u8>
        let mut buf = Vec::new();
        {
            let mut writer = Cursor::new(&mut buf);
            sector_info.write(&mut writer).unwrap();
        }

        assert_eq!(b"\x02\x01\x05\x06\x11\x12\x00\x02", buf.as_slice());
    }

    #[test]
    fn test_sector_info_deserialization() {
        let mut reader = Cursor::new(b"\x02\x01\x05\x06\x11\x12\x00\x02");
        let sector_info: SectorInfo = reader.read_le().unwrap();

        assert_eq!(sector_info.cylinder, 2);
        assert_eq!(sector_info.side, 1);
        assert_eq!(sector_info.sector_id, 5);
        assert_eq!(sector_info.sector_size, 1536);
        assert_eq!(sector_info.fdc_st1, 17);
        assert_eq!(sector_info.fdc_st2, 18);
        assert_eq!(sector_info.actual_data_length, 512);
    }

    #[test]
    fn test_track_info_serde() {
        let data = load_test_data(0x8600, 0x100)
            .expect("Failed to read test data");
        let mut reader = Cursor::new(&data);

        let track_info: TrackInfo = reader.read_le().expect("Failed to read track info");
        assert_eq!(reader.position(), 0x100, "position after reading track info should be 0x100");

        assert_eq!(track_info.cylinder_number, 3);
        assert_eq!(track_info.side_number, 1);
        assert_eq!(track_info.sector_size, 512);
        assert_eq!(track_info.num_sectors, 9);
        assert_eq!(track_info.gap3_length, 42);
        assert_eq!(track_info.sectors.len(), 9);

        let s = &track_info.sectors[0];
        assert_eq!(s.cylinder, 3);
        assert_eq!(s.side, 1);
        assert_eq!(s.sector_id, 1);
        assert_eq!(s.sector_size, 512);
        assert_eq!(s.fdc_st1, 0);
        assert_eq!(s.fdc_st2, 0);
        assert_eq!(s.actual_data_length, 512);

        let mut output = Vec::new();
        {
            let mut writer = Cursor::new(&mut output);
            track_info.write(&mut writer).unwrap();
        }

        assert_eq!(output.len(), 0x100);
        assert_eq!(output, data);
    }

    #[test]
    fn test_dsk_header_serde() {
        let data = load_test_data(0, 0x100)
            .expect("Failed to read test data");
        let mut reader = Cursor::new(&data);

        let dsk_header: DskFileHeader = reader.read_le().unwrap();
        assert_eq!(reader.position(), 0x100, "position after reading track info should be 0x100");

        assert_eq!(dsk_header.name_of_creator, *b"CPCDiskXP v2.5");
        assert_eq!(dsk_header.num_cylinders, 80);
        assert_eq!(dsk_header.num_sides, 2);
        assert_eq!(dsk_header.track_sizes, vec![19; 2 * 80]);

        // TODO: serialize back, compare
    }
}