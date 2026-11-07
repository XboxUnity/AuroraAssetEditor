#![allow(dead_code, unused_imports)]

mod manifest;
mod minimum_read;

use std::fs;
use std::path::PathBuf;

use image::RgbaImage;
use sha2::{Digest, Sha256};

pub use manifest::{Crafted, DdsSample, Fixture, Manifest, manifest};
pub use minimum_read::{
    ARGB_TEXEL, BC1_BLOCK, BC3_BLOCK, SurfaceUnit, assert_decodes_at_minimum_len, minimum_data_len,
};

fn codec_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/codec")
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

pub fn dds_bytes(sample: &DdsSample) -> Vec<u8> {
    fs::read(codec_fixtures_dir().join(&sample.file)).unwrap()
}
