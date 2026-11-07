use image::{Rgba, RgbaImage};

use super::DecodeError;
use super::layout::SurfaceLayout;
use crate::bytes::BYTES_PER_PIXEL;

/// Stores each RGBA pixel as A,R,G,B in rows of `row_pitch` bytes, zero-padded to the full surface.
pub(super) fn pack(image: &RgbaImage, layout: &SurfaceLayout) -> Vec<u8> {
    let mut surface = vec![0u8; layout.padded_len()];
    for (y, pixels) in image.rows().enumerate() {
        let row_start = y * layout.row_pitch;
        for (x, Rgba([r, g, b, a])) in pixels.enumerate() {
            let start = row_start + x * BYTES_PER_PIXEL;
            surface[start..start + BYTES_PER_PIXEL].copy_from_slice(&[*a, *r, *g, *b]);
        }
    }
    surface
}

/// Reverse of [`pack`]. The padding after the last visible pixel may be missing.
pub(super) fn unpack(
    data: &[u8],
    width: u32,
    height: u32,
    layout: &SurfaceLayout,
) -> Result<RgbaImage, DecodeError> {
    let surface = layout.read_from(data)?;
    Ok(RgbaImage::from_fn(width, height, |x, y| {
        let start = layout.unit_offset(x as usize, y as usize);
        let argb = &surface[start..start + BYTES_PER_PIXEL];
        Rgba([argb[1], argb[2], argb[3], argb[0]])
    }))
}
