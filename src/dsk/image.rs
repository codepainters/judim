use super::structs::{TrackInfo, DskFileHeader};

pub struct DskImage {
    file_header: DskFileHeader,
    tracks: Vec<TrackInfo>,
}

impl DskImage {
    // pub fn load(f: &mut File) -> Self {
    //     DskImage {}
    // }
}


