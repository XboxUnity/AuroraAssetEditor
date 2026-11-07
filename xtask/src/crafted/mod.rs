//! Crafted texture headers for the golden corpus.
//!
//! Aurora writes linear DXT1 and DXT5 textures that AuroraAsset.dll never produces itself, so
//! their header signatures are found by decoding crafted headers with the DLL and recording
//! what it accepts. The video bytes are random: any bytes are valid BC blocks.

mod case;

use std::path::Path;

use anyhow::Result;
use aurora_asset::{TextureFormat, TextureHeader};
use serde::{Deserialize, Serialize};

use crate::golden::{Paths, dll_decode, os, run_harness, sha256_hex};
use crate::minimum_read::{self, LinearTexture};
use case::{Surface, cases, crafted_header};

pub const CRAFTED_DIRECTORY: &str = "crafted";

const HEADER_BYTE_LENGTH: usize = 52;
const BYTES_PER_PIXEL: usize = 4;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Crafted {
    name: String,
    header: String,
    video: String,
    decoded_sha256: String,
    dll_accepted: bool,
    /// The case whose decoded pixels this one equals, so no pixels are stored for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    same_as: Option<String>,
    /// For a header the port decodes: whether the DLL decodes the video cut to the bytes it reads
    /// to the same pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decodes_at_minimum_len: Option<bool>,
}

impl Crafted {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn video_file(&self) -> &str {
        &self.video
    }

    pub fn decoded_file(&self) -> Option<String> {
        (self.dll_accepted && self.same_as.is_none())
            .then(|| format!("{CRAFTED_DIRECTORY}/{}.rgba", self.name))
    }
}

/// The header the DLL writes for a texture of this size and compression, used as the base to patch.
fn dll_header(
    paths: &Paths,
    work_dir: &Path,
    (width, height): (u32, u32),
    compression: &str,
) -> Result<[u8; HEADER_BYTE_LENGTH]> {
    let pixels = work_dir.join("base.argb");
    let header_path = work_dir.join("base.header");
    let video_path = work_dir.join("base.video");
    std::fs::write(
        &pixels,
        vec![0u8; width as usize * height as usize * BYTES_PER_PIXEL],
    )?;
    run_harness(
        paths,
        &[
            os("encode"),
            os("--dll"),
            paths.dll().into(),
            os("--input"),
            pixels.into(),
            os("--width"),
            width.to_string().into(),
            os("--height"),
            height.to_string().into(),
            os("--compression"),
            os(compression),
            os("--header-out"),
            header_path.clone().into(),
            os("--video-out"),
            video_path.into(),
        ],
    )?;
    let bytes = std::fs::read(&header_path)?;
    let length = bytes.len();
    bytes.try_into().map_err(|_| {
        anyhow::anyhow!("base header is {length} bytes, expected {HEADER_BYTE_LENGTH}")
    })
}

/// Decodes every crafted case with the DLL and writes the videos and decoded pixels.
pub fn write_crafted(paths: &Paths, work_dir: &Path, out_dir: &Path) -> Result<Vec<Crafted>> {
    let crafted_dir = out_dir.join(CRAFTED_DIRECTORY);
    std::fs::create_dir_all(&crafted_dir)?;

    let mut crafted: Vec<Crafted> = Vec::new();
    for case in &cases() {
        let compression = match case.surface {
            Surface::Argb => "none",
            Surface::Bc1 | Surface::Bc3 => "dxt",
        };
        let base = dll_header(paths, work_dir, (case.width, case.height), compression)?;
        let header = crafted_header(base, case);
        let video = case.video();
        let video_file = format!("{CRAFTED_DIRECTORY}/{}", case.video_file_name());
        std::fs::write(out_dir.join(&video_file), &video)?;

        let decoded = dll_decode(paths, work_dir, &case.name, &header, &video)?;
        let decoded_sha256 = decoded.as_deref().map(sha256_hex).unwrap_or_default();
        let port_decodes = !matches!(
            TextureHeader::from_bytes(header).format(),
            TextureFormat::Unsupported { .. }
        );
        let decodes_at_minimum_len = if decoded.is_some() && port_decodes {
            let texture = LinearTexture {
                header: &header,
                video: &video,
                size: (case.width, case.height),
                unit: case.surface.unit(),
            };
            Some(minimum_read::decodes_at_minimum_len(
                paths,
                work_dir,
                &case.name,
                &texture,
                &decoded_sha256,
            )?)
        } else {
            None
        };
        let same_as = case
            .baseline
            .as_ref()
            .and_then(|name| crafted.iter().find(|other| other.name == *name))
            .filter(|baseline| baseline.dll_accepted && baseline.decoded_sha256 == decoded_sha256)
            .map(|baseline| baseline.name.clone());
        if let (Some(rgba), None) = (&decoded, &same_as) {
            std::fs::write(crafted_dir.join(format!("{}.rgba", case.name)), rgba)?;
        }
        crafted.push(Crafted {
            name: case.name.clone(),
            header: hex::encode(header),
            video: video_file,
            dll_accepted: decoded.is_some(),
            decoded_sha256,
            same_as,
            decodes_at_minimum_len,
        });
    }
    Ok(crafted)
}
