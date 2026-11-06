#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use image::RgbaImage;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub fixture: Vec<Fixture>,
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

pub fn rgba_sha256(bgra_or_rgba: &[u8]) -> String {
    hex::encode(Sha256::digest(bgra_or_rgba))
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
