use image::{Rgba, RgbaImage};

use super::layout::SurfaceLayout;

const BYTES_PER_PIXEL: usize = 4;

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

/// Reverse of [`pack`]; `surface` must hold at least `layout.padded_len()` bytes.
pub(super) fn unpack(surface: &[u8], width: u32, height: u32, layout: &SurfaceLayout) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let start = y as usize * layout.row_pitch + x as usize * BYTES_PER_PIXEL;
        let argb = &surface[start..start + BYTES_PER_PIXEL];
        Rgba([argb[1], argb[2], argb[3], argb[0]])
    })
}
