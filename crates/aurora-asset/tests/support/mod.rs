#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use aurora_asset::{DecodeError, EncodedTexture, decode};
use image::RgbaImage;
use serde::Deserialize;
use sha2::{Digest, Sha256};

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

fn codec_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/codec")
}

pub fn manifest() -> Manifest {
    let text = fs::read_to_string(codec_fixtures_dir().join("manifest.toml")).unwrap();
    toml::from_str(&text).unwrap()
}

pub fn input_rgba(name: &str) -> RgbaImage {
    let path = codec_fixtures_dir()
        .join("inputs")
        .join(format!("{name}.png"));
    image::open(path).unwrap().to_rgba8()
}

pub fn dxt_video(name: &str) -> Vec<u8> {
    fs::read(codec_fixtures_dir().join("dxt").join(format!("{name}.vid"))).unwrap()
}

pub fn dxt_decoded_rgba(name: &str) -> Vec<u8> {
    fs::read(
        codec_fixtures_dir()
            .join("decoded")
            .join(format!("{name}.dxt.rgba")),
    )
    .unwrap()
}

pub fn rgba_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn header_bytes(hex_text: &str) -> [u8; 52] {
    hex::decode(hex_text).unwrap().try_into().unwrap()
}

pub fn argb_video(name: &str) -> Vec<u8> {
    fs::read(
        codec_fixtures_dir()
            .join("argb")
            .join(format!("{name}.vid")),
    )
    .unwrap()
}

pub fn crafted_video(case: &Crafted) -> Vec<u8> {
    fs::read(codec_fixtures_dir().join(&case.video)).unwrap()
}

pub fn crafted_rgba_file(name: &str) -> Vec<u8> {
    fs::read(
        codec_fixtures_dir()
            .join("crafted")
            .join(format!("{name}.rgba")),
    )
    .unwrap()
}

/// The DLL's decode of the case; a case equal to another stores no pixels of its own.
pub fn crafted_decoded_rgba(case: &Crafted) -> Vec<u8> {
    crafted_rgba_file(case.same_as.as_deref().unwrap_or(&case.name))
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

pub fn dds_bytes(sample: &DdsSample) -> Vec<u8> {
    fs::read(codec_fixtures_dir().join(&sample.file)).unwrap()
}

/// What a surface row is made of: 4x4 blocks of 8 or 16 bytes, or 4-byte texels.
#[derive(Clone, Copy)]
pub struct SurfaceUnit {
    pub texels_per_side: usize,
    pub bytes: usize,
}

pub const BC1_BLOCK: SurfaceUnit = SurfaceUnit {
    texels_per_side: 4,
    bytes: 8,
};
pub const BC3_BLOCK: SurfaceUnit = SurfaceUnit {
    texels_per_side: 4,
    bytes: 16,
};
pub const ARGB_TEXEL: SurfaceUnit = SurfaceUnit {
    texels_per_side: 1,
    bytes: 4,
};

const FETCH_0_OFFSET: usize = 28;
const PITCH_SHIFT: u32 = 22;
const PITCH_MASK: u32 = 0x1FF;
const TEXELS_PER_PITCH_UNIT: usize = 32;

/// The bytes a linear decode reads (ruling R32): every row above the last at the header's row
/// pitch, then only the visible units of the last row.
pub fn minimum_data_len(texture: &EncodedTexture, unit: SurfaceUnit) -> usize {
    let header = texture.header.to_bytes();
    let fetch_0 = u32::from_be_bytes(
        header[FETCH_0_OFFSET..FETCH_0_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    let pitch_units = ((fetch_0 >> PITCH_SHIFT) & PITCH_MASK) as usize;
    let row_pitch = pitch_units * (TEXELS_PER_PITCH_UNIT / unit.texels_per_side) * unit.bytes;
    let units_wide = (texture.header.width() as usize).div_ceil(unit.texels_per_side);
    let units_high = (texture.header.height() as usize).div_ceil(unit.texels_per_side);
    (units_high - 1) * row_pitch + units_wide * unit.bytes
}

/// The data cut to the minimum length must decode to `decoded_sha256`, and one byte less must be
/// refused as truncated.
pub fn assert_decodes_at_minimum_len(
    texture: &EncodedTexture,
    unit: SurfaceUnit,
    decoded_sha256: &str,
    label: &str,
) {
    let minimum = minimum_data_len(texture, unit);
    let cut_to = |len: usize| EncodedTexture {
        header: texture.header.clone(),
        data: texture.data[..len].to_vec(),
    };
    let decoded = decode(&cut_to(minimum)).unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        rgba_sha256(decoded.as_raw()),
        decoded_sha256,
        "{label} at {minimum} bytes"
    );
    assert_eq!(
        decode(&cut_to(minimum - 1)).err(),
        Some(DecodeError::TruncatedData {
            expected: minimum,
            actual: minimum - 1
        }),
        "{label}"
    );
}
