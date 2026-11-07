use std::fs;

use serde::Deserialize;

use super::codec_fixtures_dir;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub fixture: Vec<Fixture>,
    pub crafted: Vec<Crafted>,
    pub dds: Vec<DdsSample>,
}

#[derive(Debug, Deserialize)]
pub struct Crafted {
    pub name: String,
    pub header: String,
    pub video: String,
    pub decoded_sha256: String,
    pub dll_accepted: bool,
    pub same_as: Option<String>,
    pub decodes_at_minimum_len: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub dxt_header: String,
    pub dxt_video_sha256: String,
    pub dxt_decoded_sha256: String,
    pub argb_header: String,
    pub argb_video_sha256: String,
    pub argb_decoded_sha256: String,
    pub argb_video_stored: bool,
    pub dxt_decoded_stored: bool,
    pub dxt_decodes_at_minimum_len: bool,
    pub argb_decodes_at_minimum_len: bool,
}

#[derive(Debug, Deserialize)]
pub struct DdsSample {
    pub name: String,
    pub file: String,
    pub decoded_sha256: String,
    pub dll_accepted: bool,
    pub dll_output_unstable: bool,
    pub dll_layout_scrambled: bool,
    pub reported_len: usize,
    pub verbatim_sha256: String,
}

pub fn manifest() -> Manifest {
    let text = fs::read_to_string(codec_fixtures_dir().join("manifest.toml")).unwrap();
    toml::from_str(&text).unwrap()
}
