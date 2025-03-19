use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use std::str::FromStr;

lazy_static! {
    static ref ImageFileRe: Regex = Regex::new(r"^(?:(\d+):|:)(.*)$").unwrap();
}

const DEFAULT_USER: u8 = 0;

#[derive(Clone, Debug)]
pub struct FileLocal {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct FileImage {
    pub owner: u8,
    pub name: String,
}

#[derive(Clone, Debug)]
pub enum FileArg {
    Local(FileLocal),
    Image(FileImage),
}

impl FromStr for FileArg {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let f = if let Some(caps) = ImageFileRe.captures(s) {
            // image file (not checking filename syntax at this point, it might
            // be a glob pattern)
            let owner = if let Some(cap) = caps.get(1) {
                cap.as_str().parse()?
            } else {
                DEFAULT_USER
            };

            // TODO: check if owner is in range

            Self::Image(FileImage {
                owner,
                name: caps[2].to_string(),
            })
        } else {
            Self::Local(FileLocal { name: s.to_string() })
        };
        Ok(f)
    }
}
