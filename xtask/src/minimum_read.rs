//! Whether the DLL decodes a texture whose video stops right after the last byte it reads, so that
//! the port may accept data without the padding rows another writer could leave out.

use std::path::Path;

use anyhow::{Result, ensure};

use crate::golden::{Paths, dll_decode, sha256_hex};
use crate::surface_unit::SurfaceUnit;

const HEADER_BYTE_LENGTH: usize = 52;
const DWORD_BYTES: usize = 4;
const FETCH_0_DWORD: usize = 7;
const PITCH_SHIFT: u32 = 22;
const PITCH_BITS: u32 = 9;
const TEXELS_PER_PITCH_UNIT: usize = 32;

/// The bytes a linear decode reads: every row above the last at the header's row pitch, then
/// only the visible units of the last row.
pub fn minimum_video_len(
    header: &[u8; HEADER_BYTE_LENGTH],
    (width, height): (u32, u32),
    unit: SurfaceUnit,
) -> usize {
    let fetch_0_start = FETCH_0_DWORD * DWORD_BYTES;
    let mut fetch_0 = [0u8; DWORD_BYTES];
    fetch_0.copy_from_slice(&header[fetch_0_start..fetch_0_start + DWORD_BYTES]);
    let pitch_units = (u32::from_be_bytes(fetch_0) >> PITCH_SHIFT) & ((1 << PITCH_BITS) - 1);

    let units_per_pitch_unit = TEXELS_PER_PITCH_UNIT / unit.texels_per_side;
    let row_pitch = pitch_units as usize * units_per_pitch_unit * unit.bytes;
    let units_wide = (width as usize).div_ceil(unit.texels_per_side);
    let units_high = (height as usize).div_ceil(unit.texels_per_side);
    (units_high - 1) * row_pitch + units_wide * unit.bytes
}

/// A linear texture as the DLL wrote it or as a crafted case presents it.
pub struct LinearTexture<'a> {
    pub header: &'a [u8; HEADER_BYTE_LENGTH],
    pub video: &'a [u8],
    pub size: (u32, u32),
    pub unit: SurfaceUnit,
}

/// Decodes the video cut to [`minimum_video_len`] with the DLL; true when the pixels equal its
/// decode of the whole video, whose hash is `full_decoded_sha256`.
pub fn decodes_at_minimum_len(
    paths: &Paths,
    work_dir: &Path,
    label: &str,
    texture: &LinearTexture,
    full_decoded_sha256: &str,
) -> Result<bool> {
    let minimum = minimum_video_len(texture.header, texture.size, texture.unit);
    ensure!(
        minimum <= texture.video.len(),
        "{label}: the video is {} bytes, shorter than the {minimum} bytes a decode reads",
        texture.video.len()
    );
    let truncated = &texture.video[..minimum];
    let decoded = dll_decode(paths, work_dir, label, texture.header, truncated)?;
    Ok(decoded.is_some_and(|rgba| sha256_hex(&rgba) == full_decoded_sha256))
}
