// BC3 (DXT4/DXT5) block encoder, ported from DirectXTex `BC.cpp` (`D3DXEncodeBC3`).
//
// MIT License
//
// Copyright (c) Microsoft Corporation.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE

//! The D3DX encoder that AuroraAsset.dll's "Use Compression" path is observed to match.
//! Arithmetic follows the DLL's operation order and precision (see `x87`), so results tie out
//! bit for bit.

mod alpha;
mod color;
mod x87;

use image::RgbaImage;

use super::endian::swap_8in16;
use super::layout::SurfaceLayout;
use x87::Fpu;

const BLOCK_SIDE: usize = 4;
const TEXELS_PER_BLOCK: usize = BLOCK_SIDE * BLOCK_SIDE;
const BLOCK_BYTES: usize = 16;
const CHANNEL_MAX: f32 = 255.0;

const ALPHA_HALF: std::ops::Range<usize> = 0..8;
const COLOR_HALF: std::ops::Range<usize> = 8..16;

/// A texel as floats in `[0, 1]`: red, green, blue, alpha.
pub(crate) type Texel = [f32; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bc3Options {
    pub dither_rgb: bool,
    pub dither_alpha: bool,
    pub uniform_weighting: bool,
}

impl Bc3Options {
    /// The settings the DLL's encoder was observed to use.
    pub(crate) const DLL_BASELINE: Self = Self {
        dither_rgb: true,
        // Derived from AuroraAsset.dll for interoperability: its one dither flag covers alpha too.
        dither_alpha: true,
        uniform_weighting: false,
    };
}

/// Encodes the image as BC3 blocks in the DLL's surface layout: padded rows, zero padding, and
/// the GPU's 8in16 byte order.
pub(super) fn encode_surface(image: &RgbaImage, layout: &SurfaceLayout) -> Vec<u8> {
    let mut surface = vec![0u8; layout.padded_len()];
    let blocks_wide = (image.width() as usize).div_ceil(BLOCK_SIDE);
    let blocks_high = (image.height() as usize).div_ceil(BLOCK_SIDE);
    for block_y in 0..blocks_high {
        for block_x in 0..blocks_wide {
            let texels = block_texels(image, block_x, block_y);
            let block = encode_bc3_block(&texels, &Bc3Options::DLL_BASELINE);
            let start = block_y * layout.row_pitch + block_x * BLOCK_BYTES;
            surface[start..start + BLOCK_BYTES].copy_from_slice(&block);
        }
    }
    swap_8in16(&mut surface);
    surface
}

/// The 16 texels of a block, row-major. A partial block fills its missing columns and rows from
/// the ones inside the image.
fn block_texels(image: &RgbaImage, block_x: usize, block_y: usize) -> [Texel; TEXELS_PER_BLOCK] {
    let columns_inside = (image.width() as usize - block_x * BLOCK_SIDE).min(BLOCK_SIDE);
    let rows_inside = (image.height() as usize - block_y * BLOCK_SIDE).min(BLOCK_SIDE);
    std::array::from_fn(|index| {
        let column = source_position(index % BLOCK_SIDE, columns_inside);
        let row = source_position(index / BLOCK_SIDE, rows_inside);
        let x = block_x * BLOCK_SIDE + column;
        let y = block_y * BLOCK_SIDE + row;
        let [red, green, blue, alpha] = image.get_pixel(x as u32, y as u32).0;
        [red, green, blue, alpha].map(channel_to_unit)
    })
}

// Derived from AuroraAsset.dll for interoperability: missing positions copy from 0, 0, 0, 1.
const PARTIAL_BLOCK_SOURCE: [usize; BLOCK_SIDE] = [0, 0, 0, 1];

/// The column (or row) of the block that a position outside the image takes its texel from.
fn source_position(position: usize, positions_inside: usize) -> usize {
    let mut source = position;
    while source >= positions_inside {
        source = PARTIAL_BLOCK_SOURCE[source];
    }
    source
}

// Derived from AuroraAsset.dll for interoperability: bytes load as `n * (1/255)`, not `n / 255`.
fn channel_to_unit(channel: u8) -> f32 {
    const UNIT_PER_LEVEL: f32 = 1.0 / CHANNEL_MAX;
    f32::from(channel) * UNIT_PER_LEVEL
}

/// One 16-byte block in little-endian D3D order: the alpha half, then the color half.
pub(crate) fn encode_bc3_block(
    texels: &[Texel; TEXELS_PER_BLOCK],
    options: &Bc3Options,
) -> [u8; BLOCK_BYTES] {
    let mut block = [0u8; BLOCK_BYTES];
    block[ALPHA_HALF].copy_from_slice(&alpha::encode_alpha_block(texels, options));
    block[COLOR_HALF].copy_from_slice(&color::encode_color_block(texels, options));
    block
}

/// How much of each palette entry comes from the first and second endpoint: entry `i` of `n`
/// evenly spaced steps is `(n - 1 - i) / (n - 1)` of the first plus `i / (n - 1)` of the second.
/// Entries past `step_count` stay zero.
fn end_weights<const MAX_STEPS: usize>(step_count: usize) -> ([f32; MAX_STEPS], [f32; MAX_STEPS]) {
    let last = step_count - 1;
    let first = std::array::from_fn(|step| {
        if step < step_count {
            (last - step) as f32 / last as f32
        } else {
            0.0
        }
    });
    let second = std::array::from_fn(|step| {
        if step < step_count {
            step as f32 / last as f32
        } else {
            0.0
        }
    });
    (first, second)
}

const NEIGHBOUR_RIGHT_WEIGHT: f32 = 7.0 / 16.0;
const NEIGHBOUR_BELOW_LEFT_WEIGHT: f32 = 3.0 / 16.0;
const NEIGHBOUR_BELOW_WEIGHT: f32 = 5.0 / 16.0;
const NEIGHBOUR_BELOW_RIGHT_WEIGHT: f32 = 1.0 / 16.0;
const LAST_ROW_START: usize = TEXELS_PER_BLOCK - BLOCK_SIDE;

/// A texel's accumulated error after it receives `weight` of a neighbour's `difference`.
// Derived from AuroraAsset.dll for interoperability: its dithering loops run with the FPU truncating.
fn diffuse_error(error: f32, difference: f32, weight: f32) -> f32 {
    (Fpu::RoundTowardZero.load(difference) * weight + error).store()
}

/// Floyd-Steinberg error diffusion inside a block: calls `add_error(target, weight)` for each
/// texel that receives a share of the error of `texel`, in the original's order.
fn spread_error(texel: usize, mut add_error: impl FnMut(usize, f32)) {
    let column = texel % BLOCK_SIDE;
    let has_right = column != BLOCK_SIDE - 1;
    let has_row_below = texel < LAST_ROW_START;
    if has_right {
        add_error(texel + 1, NEIGHBOUR_RIGHT_WEIGHT);
    }
    if has_row_below {
        if column != 0 {
            add_error(texel + BLOCK_SIDE - 1, NEIGHBOUR_BELOW_LEFT_WEIGHT);
        }
        add_error(texel + BLOCK_SIDE, NEIGHBOUR_BELOW_WEIGHT);
        if has_right {
            add_error(texel + BLOCK_SIDE + 1, NEIGHBOUR_BELOW_RIGHT_WEIGHT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(texel: Texel) -> [Texel; TEXELS_PER_BLOCK] {
        [texel; TEXELS_PER_BLOCK]
    }

    #[test]
    fn opaque_block_stores_full_alpha_with_no_indices() {
        let block = encode_bc3_block(&solid([0.2, 0.4, 0.6, 1.0]), &Bc3Options::DLL_BASELINE);
        assert_eq!(block[ALPHA_HALF], [0xff, 0xff, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn solid_color_block_has_equal_endpoints_and_zero_indices() {
        let block = encode_bc3_block(&solid([1.0, 1.0, 1.0, 1.0]), &Bc3Options::DLL_BASELINE);
        assert_eq!(block[COLOR_HALF], [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
    }

    #[test]
    fn alpha_with_opaque_texels_uses_six_step_mode_low_endpoint_first() {
        let mut texels = solid([0.0, 0.0, 0.0, 1.0]);
        for texel in texels.iter_mut().take(8) {
            texel[3] = channel_to_unit(0x80);
        }
        let block = encode_bc3_block(&texels, &Bc3Options::DLL_BASELINE);
        assert_eq!((block[0], block[1]), (0x80, 0xff));
    }

    #[test]
    fn spread_error_skips_neighbours_outside_the_block() {
        let mut corner_targets = Vec::new();
        spread_error(15, |target, _| corner_targets.push(target));
        assert!(corner_targets.is_empty());

        let mut top_left_targets = Vec::new();
        spread_error(0, |target, _| top_left_targets.push(target));
        assert_eq!(top_left_targets, [1, 4, 5]);

        let mut interior_targets = Vec::new();
        spread_error(5, |target, _| interior_targets.push(target));
        assert_eq!(interior_targets, [6, 8, 9, 10]);
    }
}
