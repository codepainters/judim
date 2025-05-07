use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;
use std::str::FromStr;

use crate::cpm::MAX_USER_ID;

lazy_static! {
    static ref ImageFileRe: Regex = Regex::new(r"^(?:(\d+):|:)(.*)$").unwrap();
}

const DEFAULT_USER: u8 = 0;

// FIXME: FileArg could use Option<&str>, but for reasons I don't know yet,
//   &Path is not Clone. Sticking to owned types for now.

#[derive(Clone, Debug)]
pub enum FileArg {
    Local { path: PathBuf },
    Image { owner: u8, name: Option<String> },
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

            if owner > MAX_USER_ID {
                bail!("User ID {} is not in range 0..{}", owner, MAX_USER_ID);
            }

            // normalize empty name to None, for "dir mode"
            let name = caps[2].trim();
            let name = if name.is_empty() { None } else { Some(name.to_owned()) };
            Self::Image { owner, name }
        } else {
            let path = PathBuf::from(s.trim());
            Self::Local { path }
        };
        Ok(f)
    }
}

impl FileArg {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local { .. })
    }

    pub fn is_dir(&self) -> bool {
        match self {
            Self::Local { path } => path.is_dir(),
            Self::Image { owner: _, name } => name.is_none(),
        }
    }
}
