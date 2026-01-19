use anyhow::{bail, Error};
use binrw::BinReaderExt;
use binrw::{binrw, BinWriterExt};
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Read, Write};

// References:
// - https://sinclair.wiki.zxnet.co.uk/wiki/Spectrum_tape_interface
// - https://sinclair.wiki.zxnet.co.uk/wiki/TAP_format

/// Type of ZX Spectrum file
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
#[binrw]
#[brw(repr=u8)]
pub enum SpeccyFileType {
    /// Program in Basic
    Program = 0,
    /// Array of numbers
    NumArray = 1,
    /// Array of strings
    ChrArray = 2,
    /// Raw memory content
    Code = 3,
}

impl SpeccyFileType {
    /// Returns 3-characters extension for the file type, as used by Junior filesystem.
    pub fn extension(&self) -> &'static str {
        match self {
            SpeccyFileType::Program => "prg",
            SpeccyFileType::NumArray => "arr",
            SpeccyFileType::ChrArray => "str",
            SpeccyFileType::Code => "cod",
        }
    }
}

impl fmt::Display for SpeccyFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeccyFileType::Program => write!(f, "BASIC Program"),
            SpeccyFileType::NumArray => write!(f, "Number Array"),
            SpeccyFileType::ChrArray => write!(f, "String Array"),
            SpeccyFileType::Code => write!(f, "Code/bytes"),
        }
    }
}

/// ZX Spectrum file header
#[derive(Debug)]
#[binrw]
#[brw(little)]
pub struct SpeccyFileHeader {
    pub file_type: SpeccyFileType,
    name: [u8; 10],
    pub length: u16,
    // for Program - autostart line number, load address for Code
    pub param1: u16,
    // for Program - start of var area (relative to program start)
    pub param2: u16,
}

impl SpeccyFileHeader {
    pub fn name(&self) -> &[u8] {
        let end = self
            .name
            .iter()
            .rposition(|&b| b != 0x20)
            .map(|pos| pos + 1)
            .unwrap_or(0);
        &self.name[0..end]
    }
}

impl SpeccyFile {
    /// Reads a single ZX Spectrum file from a file.
    ///
    /// It expects to see a single ZX Spectrum file header at the start of the file,
    /// followed by the file data - as stored on Junior disks. Note: the file might be longer
    /// than data in it due to the way CP/M filesystem works (size is a multiple of 128 bytes
    /// on Junior).
    pub fn read(f: &mut File) -> Result<Self, Error> {
        let header: SpeccyFileHeader = f.read_le()?;
        let mut data: Vec<u8> = vec![0; header.length as usize];
        f.read_to_end(&mut data)?;

        Self::from_header_and_data(header, data)
    }

    /// Reads a single ZX Spectrum file from a tape file.
    ///
    /// It returns Some(None), if f was at the end already.
    pub fn read_from_tap(f: &mut File) -> Result<Option<Self>, Error> {
        // before the actual header there are always 3 bytes of size (17 bytes) and
        // 00 flag indicating header
        let mut size_and_flag = [0u8; 3];
        if Self::read_up_to(f, &mut size_and_flag)? == 0 {
            return Ok(None);
        }
        if size_and_flag != *b"\x13\x00\x00" {
            bail!(
                "Invalid header marker: {}",
                size_and_flag[0..3]
                    .iter()
                    .map(|&b| format!("{:02X}", b))
                    .collect::<Vec<String>>()[..]
                    .join("70 ")
            );
        }

        let mut header_bytes = [0u8; 17];
        f.read_exact(&mut header_bytes)?;
        let header_checksum = header_bytes.iter().fold(0u8, |acc, &b| acc ^ b);
        let expected_checksum: u8 = f.read_le()?;
        if expected_checksum != header_checksum {
            bail!("Header checksum mismatch: {} {}", expected_checksum, header_checksum);
        }

        // TODO: it might make more sense do diss binrw and let SpeccyFileHeader work with bytes
        let header: SpeccyFileHeader = Cursor::new(&header_bytes).read_le()?;

        // the next 3 bytes contain data size and 0xFF flag
        f.read_exact(&mut size_and_flag)?;
        if size_and_flag[2] != 0xFF {
            bail!("Invalid data marker");
        }

        // Note: -2, because the size includes flag and checksum
        let data_size = u16::from_le_bytes(size_and_flag[0..2].try_into().expect("Invalid size")) - 2;

        let mut data = vec![0; data_size as usize];
        f.read_exact(&mut data)?;
        let expected_checksum: u8 = f.read_le()?;
        // checksum includes flag byte!
        let actual_checksum = data.iter().fold(0u8, |acc, &b| acc ^ b) ^ 0xFF;
        if actual_checksum != expected_checksum {
            bail!("Checksum mismatch");
        }

        let f = Self::from_header_and_data(header, data)?;
        Ok(Some(f))
    }

    /// Loads all Speccy files from a given .tap file handle.
    pub fn load_tap_file(f: &mut File) -> Result<Vec<Self>, Error> {
        let mut files: Vec<Self> = Vec::new();
        while let Some(file) = Self::read_from_tap(f)? {
            files.push(file);
        }
        Ok(files)
    }

    pub fn write_header(&self, f: &mut File) -> Result<(), Error> {
        f.write_le(&self.header())?;
        Ok(())
    }

    pub fn write_raw_data(&self, f: &mut File) -> Result<(), Error> {
        f.write_all(&self.data())?;
        Ok(())
    }

    pub fn name(&self) -> String {
        let raw_name = self.header().name();
        String::from_utf8_lossy(raw_name).to_string()
    }

    pub fn file_type(&self) -> SpeccyFileType {
        self.header().file_type
    }

    pub fn size(&self) -> usize {
        self.header().length as usize
    }

    fn from_header_and_data(header: SpeccyFileHeader, data: Vec<u8>) -> Result<SpeccyFile, Error> {
        let f = match header.file_type {
            SpeccyFileType::Program => SpeccyFile::Program(SFProgram::from_header_and_data(header, data)?),
            SpeccyFileType::NumArray => SpeccyFile::NumArray(SFNumArray::from_header_and_data(header, data)?),
            SpeccyFileType::ChrArray => SpeccyFile::StrArray(SFStrArray::from_header_and_data(header, data)?),
            SpeccyFileType::Code => SpeccyFile::Code(SFCode::from_header_and_data(header, data)?),
        };
        Ok(f)
    }

    fn read_up_to(f: &mut File, buf: &mut [u8]) -> Result<usize, Error> {
        let mut offset = 0;
        while offset < buf.len() {
            let n = f.read(&mut buf[offset..])?;
            if n == 0 {
                break;
            }
            offset += n;
        }
        Ok(offset)
    }

    fn header(&self) -> &SpeccyFileHeader {
        match self {
            SpeccyFile::Program(p) => &p.header,
            SpeccyFile::NumArray(n) => &n.header,
            SpeccyFile::StrArray(s) => &s.header,
            SpeccyFile::Code(c) => &c.header,
        }
    }

    fn data(&self) -> &[u8] {
        match self {
            SpeccyFile::Program(p) => &p.data,
            SpeccyFile::NumArray(n) => &n.data,
            SpeccyFile::StrArray(s) => &s.data,
            SpeccyFile::Code(c) => &c.data,
        }
    }
}

pub enum SpeccyFile {
    Program(SFProgram),
    NumArray(SFNumArray),
    StrArray(SFStrArray),
    Code(SFCode),
}

pub struct SFProgram {
    header: SpeccyFileHeader,
    data: Vec<u8>,
}

impl SFProgram {
    fn from_header_and_data(header: SpeccyFileHeader, data: Vec<u8>) -> Result<Self, Error> {
        Ok(Self { header, data })
    }

    pub fn get_autostart_line(&self) -> Option<u16> {
        if self.header.param1 < 0x4000 {
            Some(self.header.param1)
        } else {
            None
        }
    }

    pub fn vars_offset(&self) -> u16 {
        self.header.param2
    }

    pub fn disable_autorun(&mut self) {
        // Note: I don't like the mutability here, I'd rather mask it at saving time.
        self.header.param1 = 0x8000;
    }
}

pub struct SFNumArray {
    header: SpeccyFileHeader,
    data: Vec<u8>,
}

impl SFNumArray {
    fn from_header_and_data(header: SpeccyFileHeader, data: Vec<u8>) -> Result<Self, Error> {
        Ok(Self { header, data })
    }
}

pub struct SFStrArray {
    header: SpeccyFileHeader,
    data: Vec<u8>,
}

impl SFStrArray {
    fn from_header_and_data(header: SpeccyFileHeader, data: Vec<u8>) -> Result<Self, Error> {
        Ok(Self { header, data })
    }
}

pub struct SFCode {
    header: SpeccyFileHeader,
    data: Vec<u8>,
}

impl SFCode {
    fn from_header_and_data(header: SpeccyFileHeader, data: Vec<u8>) -> Result<Self, Error> {
        Ok(Self { header, data })
    }

    pub fn load_address(&self) -> u16 {
        self.header.param1
    }
}

#[cfg(test)]
mod tests {
    use super::{SpeccyFileHeader, SpeccyFileType};
    use binrw::BinReaderExt;
    use std::io::Cursor;

    #[test]
    fn test_speccy_file_header_parse() {
        let mut reader = Cursor::new(b"\x00\x41\x42\x20\x20\x20\x20\x20\x20\x20\x20\x01\x30\x02\x40\x03\x50");
        let h: SpeccyFileHeader = reader.read_le().unwrap();

        assert_eq!(h.file_type, SpeccyFileType::Program);
        assert_eq!(h.name(), "AB".as_bytes());
        assert_eq!(h.length, 12289);
        assert_eq!(h.param1, 16386);
        assert_eq!(h.param2, 20483);
    }
}
