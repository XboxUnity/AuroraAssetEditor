use std::path::Path;

use anyhow::{Result, ensure};
use image::RgbaImage;

use super::compression::Compression;
use super::{Paths, bgra_to_rgba, os, rgba_to_argb, run_harness, sha256_hex};
use crate::minimum_read::{self, LinearTexture};

const HEADER_BYTE_LENGTH: usize = 52;

pub(super) struct Encoded {
    pub(super) header: [u8; HEADER_BYTE_LENGTH],
    pub(super) video: Vec<u8>,
    pub(super) decoded_rgba: Vec<u8>,
    pub(super) decodes_at_minimum_len: bool,
}

pub(super) fn encode_and_decode(
    paths: &Paths,
    work_dir: &Path,
    image: &RgbaImage,
    compression: Compression,
) -> Result<Encoded> {
    let (width, height) = image.dimensions();
    let argb_path = work_dir.join("input.argb");
    let header_path = work_dir.join("out.header");
    let video_path = work_dir.join("out.video");
    let decoded_path = work_dir.join("out.bgra");
    std::fs::write(&argb_path, rgba_to_argb(image.as_raw()))?;

    run_harness(
        paths,
        &[
            os("encode"),
            os("--dll"),
            paths.dll().into(),
            os("--input"),
            argb_path.into(),
            os("--width"),
            width.to_string().into(),
            os("--height"),
            height.to_string().into(),
            os("--compression"),
            os(compression.harness_name()),
            os("--header-out"),
            header_path.clone().into(),
            os("--video-out"),
            video_path.clone().into(),
        ],
    )?;
    let decode_report = run_harness(
        paths,
        &[
            os("decode"),
            os("--dll"),
            paths.dll().into(),
            os("--header"),
            header_path.clone().into(),
            os("--video"),
            video_path.clone().into(),
            os("--out"),
            decoded_path.clone().into(),
        ],
    )?;
    ensure!(
        decode_report.trim() == format!("{width} {height}"),
        "decoded size {:?} differs from the input size {width}x{height}",
        decode_report.trim()
    );

    let header = std::fs::read(&header_path)?;
    let header: [u8; HEADER_BYTE_LENGTH] = header.try_into().map_err(|header: Vec<u8>| {
        anyhow::anyhow!(
            "header is {} bytes, expected {HEADER_BYTE_LENGTH}",
            header.len()
        )
    })?;
    let video = std::fs::read(&video_path)?;
    let decoded_rgba = bgra_to_rgba(&std::fs::read(&decoded_path)?);
    let texture = LinearTexture {
        header: &header,
        video: &video,
        size: (width, height),
        unit: compression.surface_unit(),
    };
    let decodes_at_minimum_len = minimum_read::decodes_at_minimum_len(
        paths,
        work_dir,
        compression.harness_name(),
        &texture,
        &sha256_hex(&decoded_rgba),
    )?;
    Ok(Encoded {
        header,
        video,
        decoded_rgba,
        decodes_at_minimum_len,
    })
}
