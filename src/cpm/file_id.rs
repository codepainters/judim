/// FileId represents a CP/M filesystem file ID.
///
/// File in CP/M is identified by a name (max 8 ASCII characters),
/// extension (max 3 ASCII characters), as well as user owning the file.
/// I.e. there can be multiple files with the same name, owned by different users.
use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::bytes::Regex;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum FilenameMode {
    AsIs,
    Normalized,
}

pub const MAX_USER_ID: u8 = 15;
pub const MAX_NAME_LEN: usize = 8;
pub const MAX_EXT_LEN: usize = 3;

lazy_static! {
    static ref ValidNameRe: Regex = Regex::new(r"^[A-Za-z0-9!#\$%&'\(\)\-@^_{\}~]+ *$").unwrap();
    static ref ValidExtRe: Regex = Regex::new(r"^[A-Za-z0-9!#\$%&'\(\)\-@^_{\}~]* *$").unwrap();
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub struct FileId {
    pub user: u8,
    pub name: [u8; 8],
    pub extension: [u8; 3],
}

impl FileId {
    /// Create FileId from string and user ID.
    ///
    /// Note: CP/M filesystem is case-sensitive (contrary to common belief), but CCP converts
    /// all names to upper case. We mimic it here - if mode is Normalized, name is converted
    /// to uppercase, use it when creating new directory entries.
    ///
    /// Deleted entries can't be created using this function.    
    pub fn new_with_filename(user: u8, filename: &str, mode: FilenameMode) -> Result<Self> {
        if user > MAX_USER_ID {
            bail!("invalid user ID: {}", user);
        }

        if let Some((name, extension)) = Self::parse_filename(filename) {
            let mut id = FileId {
                user,
                name: [0x20; MAX_NAME_LEN],
                extension: [0x20; MAX_EXT_LEN],
            };

            Self::str_to_padded_bytes(&mut id.name, name, mode);
            Self::str_to_padded_bytes(&mut id.extension, extension, mode);

            if !ValidNameRe.is_match(&id.name) || !ValidExtRe.is_match(&id.extension) {
                bail!("invalid name: {:?}.{:?}", name, extension);
            }

            Ok(id)
        } else {
            bail!(format!("invalid filename: {}", filename));
        }
    }

    /// Create FileId instance by parsing first 12 bytes of directory entry.
    ///
    /// Note: flags (stored as MSB of extension bytes) are not parsed here.
    pub fn from_bytes(bytes: &[u8; 12]) -> Result<Self> {
        let user = bytes[0];
        let name = &bytes[1..1 + MAX_NAME_LEN];
        let extension = &bytes[1 + MAX_NAME_LEN..1 + MAX_NAME_LEN + MAX_EXT_LEN];

        let mut id = FileId {
            user,
            name: [0x20; MAX_NAME_LEN],
            extension: [0x20; MAX_EXT_LEN],
        };

        id.name.copy_from_slice(name);
        id.extension.copy_from_slice(extension);

        // Note: perform this validation only for non-deleted entries.
        // Deleted ones might not be valid, or might be all 0xE5.
        if user != 0xE5 {
            // note: name is not used for flags, so it should be ASCII without trimming
            id.extension.iter_mut().for_each(|b| *b &= 0x7F);

            if user > MAX_USER_ID {
                bail!("invalid user ID: {}", user);
            }
            if !ValidNameRe.is_match(&id.name) || !ValidExtRe.is_match(&id.extension) {
                bail!("invalid name: {:?}.{:?}", name, extension);
            }
        }

        Ok(id)
    }

    /// Serialize data back (in place) to a given mutable slice.
    ///
    /// Note: for deleted entries we only set the first byte, leaving everything else
    /// untouched. This is to preserve deleted entries as is when serializing the whole image
    /// back to dsk file.
    pub fn to_bytes(&self, bytes: &mut [u8]) {
        bytes[0] = self.user;
        if self.user != 0xE5 {
            bytes[1..1 + MAX_NAME_LEN].copy_from_slice(&self.name);
            bytes[1 + MAX_NAME_LEN..1 + MAX_NAME_LEN + MAX_EXT_LEN].copy_from_slice(&self.extension);
        }
    }

    pub fn filename(&self) -> String {
        let name = String::from_utf8_lossy(&self.name);
        let extension = String::from_utf8_lossy(&self.extension);
        format!("{}.{}", name.trim_end(), extension.trim_end())
    }

    fn parse_filename(filename: &str) -> Option<(&str, &str)> {
        // make sure it's a valid 8.3 name
        if let Some(parts) = filename.split_once('.') {
            // ensure it is at most 8 + 3 ASCII characters
            if parts.1.contains('.')
                || parts.0.len() > MAX_NAME_LEN
                || parts.1.len() > MAX_EXT_LEN
                || !(parts.0.is_ascii() && parts.1.is_ascii())
            {
                return None;
            }

            return Some(parts);
        }
        None
    }

    fn str_to_padded_bytes(dst: &mut [u8], n: &str, mode: FilenameMode) {
        let mut tmp = n.to_string();
        if mode == FilenameMode::Normalized {
            tmp.make_ascii_uppercase();
        };
        let bytes = tmp.as_bytes();
        dst[..bytes.len()].copy_from_slice(bytes);
    }
}

#[cfg(test)]
mod tests {
    use crate::cpm::file_id::FilenameMode::{AsIs, Normalized};
    use crate::cpm::file_id::{FileId, FilenameMode};

    #[test]
    fn test_new_valid_case_as_is() {
        let id = FileId::new_with_filename(1, "FoO.Pas", AsIs).unwrap();
        assert_eq!(id.user, 1);
        assert_eq!(id.name, *b"FoO     ");
        assert_eq!(id.extension, *b"Pas");
    }

    #[test]
    fn test_new_valid_case_norm() {
        let id = FileId::new_with_filename(1, "FoO.Pas", Normalized).unwrap();
        assert_eq!(id.user, 1);
        assert_eq!(id.name, *b"FOO     ");
        assert_eq!(id.extension, *b"PAS");
    }

    #[test]
    fn test_new_invalid_name() {
        assert!(FileId::new_with_filename(1, "a.b.c", FilenameMode::Normalized).is_err());
        assert!(FileId::new_with_filename(1, "a.bdec", FilenameMode::Normalized).is_err());
        assert!(FileId::new_with_filename(1, "abcdefghi.bec", FilenameMode::Normalized).is_err());
        assert!(FileId::new_with_filename(1, "abcd😀.bec", FilenameMode::Normalized).is_err());
        assert!(FileId::new_with_filename(1, "abcd.b😀", FilenameMode::Normalized).is_err());

        // these use ASCII but outside allowed character subset
        assert!(FileId::new_with_filename(1, "abcd.b+", FilenameMode::Normalized).is_err());
        assert!(FileId::new_with_filename(1, "a+bcd.b", FilenameMode::Normalized).is_err());
    }

    #[test]
    fn test_new_invalid_user() {
        assert!(FileId::new_with_filename(0, "a.b", FilenameMode::Normalized).is_ok());
        assert!(FileId::new_with_filename(15, "a.b", FilenameMode::Normalized).is_ok());
        assert!(FileId::new_with_filename(16, "a.b", FilenameMode::Normalized).is_err());
        // creating deleted files is disallowed
        assert!(FileId::new_with_filename(0xE5, "a.b", FilenameMode::Normalized).is_err());
    }

    #[test]
    fn test_to_bytes() {
        let id = FileId::new_with_filename(3, "FoO.Pas", Normalized).unwrap();
        let mut bytes = [0; 12];
        id.to_bytes(&mut bytes);
        assert_eq!(bytes, *b"\x03FOO     PAS");
    }

    #[test]
    fn test_to_bytes_deleted() {
        let mut id = FileId::new_with_filename(3, "FoO.Pas", Normalized).unwrap();
        id.user = 0xE5;
        let mut bytes = b"0123456789AB".clone();
        id.to_bytes(&mut bytes);
        assert_eq!(bytes, *b"\xE5123456789AB");
    }

    #[test]
    fn test_from_bytes_invalid_user() {
        assert!(FileId::from_bytes(b"A123456789AB").is_err());
    }

    #[test]
    fn test_from_bytes_valid_case() {
        let id = FileId::from_bytes(b"\x00TesT    zX ");
        assert!(id.is_ok());

        let id = id.unwrap();
        assert_eq!(id.user, 0);
        assert_eq!(id.filename(), "TesT.zX");
    }

    #[test]
    fn test_from_bytes_name_validation() {
        // space inside name
        assert!(FileId::from_bytes(b"\x00Te T    zX ").is_err());
        // dot inside name
        assert!(FileId::from_bytes(b"\x00Te.T    zX ").is_err());
        // empty name
        assert!(FileId::from_bytes(b"\x00        zX ").is_err());
        // name with byte >127
        assert!(FileId::from_bytes(b"\x00\xAA       zX ").is_err());

        // empty extension is OK
        assert!(FileId::from_bytes(b"\x00TeeT       ").is_ok());
        // so is extension with >127 code (MSB is for flags)
        assert!(FileId::from_bytes(b"\x00TeeT    \xC1  ").is_ok());
    }
}
