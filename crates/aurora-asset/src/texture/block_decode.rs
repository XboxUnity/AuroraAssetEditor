use image::{Rgba, RgbaImage};

use super::DecodeError;
use super::channel::{expand_565, unit_from_byte};
use super::channel_order::ChannelOrder;
use super::gpu_endian::GpuEndian;
use super::surface_layout::SurfaceLayout;
use crate::bytes::{BLOCK_SIDE, CHANNEL_MAX, block_array};

const TEXELS_PER_BLOCK: usize = BLOCK_SIDE * BLOCK_SIDE;
const BC1_BLOCK_BYTES: usize = 8;
const BC3_BLOCK_BYTES: usize = 16;

const ALPHA_ENDPOINTS: std::ops::Range<usize> = 0..2;
const ALPHA_INDEX_BYTES: std::ops::Range<usize> = 2..8;
const COLOR_BLOCK_START: usize = 8;
/// Byte ranges inside a BC1 block, which is also the color half of a BC3 block.
const COLOR_FIRST_ENDPOINT: std::ops::Range<usize> = 0..2;
const COLOR_SECOND_ENDPOINT: std::ops::Range<usize> = 2..4;
const COLOR_INDEX_BYTES: std::ops::Range<usize> = 4..8;
const ALPHA_INDEX_BITS: u32 = 3;
const COLOR_INDEX_BITS: u32 = 2;

const OPAQUE: f32 = 1.0;

type Texel = [u8; 4];
type FloatColor = [f32; 3];
/// Channels as the DLL holds them between decoding a block and writing pixels: `f32`, 0 to 1.
type FloatTexel = [f32; 4];

const TRANSPARENT_BLACK: FloatTexel = [0.0, 0.0, 0.0, 0.0];

#[derive(Clone, Copy)]
pub(super) enum BlockCodec {
    Bc1,
    /// With `divide_alpha`, colour is divided by alpha: the quirk of the DLL's DXT4 decode,
    /// kept so that decoded pixels equal what Aurora's tooling shows.
    Bc3 {
        divide_alpha: bool,
    },
}

impl BlockCodec {
    fn block_bytes(self) -> usize {
        match self {
            Self::Bc1 => BC1_BLOCK_BYTES,
            Self::Bc3 { .. } => BC3_BLOCK_BYTES,
        }
    }

    fn decode_block(self, block: &[u8]) -> [FloatTexel; TEXELS_PER_BLOCK] {
        match self {
            Self::Bc1 => decode_bc1_block_values(&block_array(block)),
            Self::Bc3 { divide_alpha } => {
                decode_bc3_block_values(&block_array(block), divide_alpha)
            }
        }
    }
}

/// Decodes a BC1 colour block (little-endian words) to RGBA texels, row-major within the block.
pub(crate) fn decode_bc1_block(block: &[u8; 8]) -> [Texel; TEXELS_PER_BLOCK] {
    decode_bc1_block_values(block).map(texel_to_bytes)
}

fn decode_bc1_block_values(block: &[u8; 8]) -> [FloatTexel; TEXELS_PER_BLOCK] {
    let colors = dxt1_palette(block);
    let indices = color_indices(block);
    std::array::from_fn(|texel| colors[indices[texel]])
}

fn opaque([red, green, blue]: FloatColor) -> FloatTexel {
    [red, green, blue, OPAQUE]
}

/// The palette of a BC1 block and of the colour half of a BC3 block: four colours when the first
/// endpoint is greater than the second, otherwise the two endpoints, their midpoint and
/// transparent black.
fn dxt1_palette(block: &[u8; 8]) -> [FloatTexel; 4] {
    let first_packed = u16::from_le_bytes(block_array(&block[COLOR_FIRST_ENDPOINT]));
    let second_packed = u16::from_le_bytes(block_array(&block[COLOR_SECOND_ENDPOINT]));
    let first = expand_565(first_packed);
    let second = expand_565(second_packed);
    if first_packed > second_packed {
        [
            opaque(first),
            opaque(second),
            opaque(step_between(first, second, 1.0 / 3.0)),
            opaque(step_between(first, second, 2.0 / 3.0)),
        ]
    } else {
        [
            opaque(first),
            opaque(second),
            opaque(step_between(first, second, 0.5)),
            TRANSPARENT_BLACK,
        ]
    }
}

// Derived from AuroraAsset.dll for interoperability: SSE `f32` lerp.
fn step_between(first: FloatColor, second: FloatColor, weight: f32) -> FloatColor {
    std::array::from_fn(|channel| first[channel] + (second[channel] - first[channel]) * weight)
}

/// Decodes a BC3 block to RGBA texels, row-major within the block.
pub(crate) fn decode_bc3_block(block: &[u8; 16], divide_alpha: bool) -> [Texel; TEXELS_PER_BLOCK] {
    decode_bc3_block_values(block, divide_alpha).map(texel_to_bytes)
}

fn decode_bc3_block_values(block: &[u8; 16], divide_alpha: bool) -> [FloatTexel; TEXELS_PER_BLOCK] {
    let color_block: [u8; 8] = std::array::from_fn(|offset| block[COLOR_BLOCK_START + offset]);
    let colors = dxt1_palette(&color_block).map(|[red, green, blue, _]| [red, green, blue]);
    let color_indices = color_indices(&color_block);
    let alphas = alpha_ramp(block[ALPHA_ENDPOINTS][0], block[ALPHA_ENDPOINTS][1]);
    let alpha_indices = alpha_indices(block);
    std::array::from_fn(|texel| {
        let color = colors[color_indices[texel]];
        let alpha = alphas[alpha_indices[texel]];
        let [red, green, blue] = if divide_alpha {
            color.map(|channel| divide_by_alpha(channel, alpha))
        } else {
            color
        };
        [red, green, blue, alpha]
    })
}

/// Colour divided by alpha, at most 1; transparent texels become black.
// Derived from AuroraAsset.dll for interoperability: SSE `f32` `min((1 / alpha) * colour, 1)`.
fn divide_by_alpha(channel: f32, alpha: f32) -> f32 {
    if alpha == 0.0 {
        return 0.0;
    }
    ((1.0 / alpha) * channel).min(OPAQUE)
}

fn texel_to_bytes(values: FloatTexel) -> Texel {
    values.map(channel_to_byte)
}

/// A channel from 0 to 1 as an 8-bit value, rounded to nearest with ties to even.
// Derived from AuroraAsset.dll for interoperability: SSE `f32` `value * 255`, then `cvtps2dq`.
fn channel_to_byte(value: f32) -> u8 {
    (value * CHANNEL_MAX).round_ties_even() as u8
}

/// The bits of `value` selected by `mask` after shifting right by `shift`.
fn bit_field(value: u64, shift: u32, mask: u64) -> u64 {
    (value >> shift) & mask
}

/// The `texel`-th packed palette index, `bits_per_index` wide, counting from the low bits.
fn index_at(packed: u64, bits_per_index: u32, texel: usize) -> usize {
    let mask = (1 << bits_per_index) - 1;
    bit_field(packed, bits_per_index * texel as u32, mask) as usize
}

fn color_indices(color_block: &[u8; 8]) -> [usize; TEXELS_PER_BLOCK] {
    let bits = u32::from_le_bytes(block_array(&color_block[COLOR_INDEX_BYTES]));
    std::array::from_fn(|texel| index_at(u64::from(bits), COLOR_INDEX_BITS, texel))
}

fn alpha_indices(block: &[u8; 16]) -> [usize; TEXELS_PER_BLOCK] {
    let mut bytes = [0u8; size_of::<u64>()];
    bytes[..ALPHA_INDEX_BYTES.len()].copy_from_slice(&block[ALPHA_INDEX_BYTES]);
    let bits = u64::from_le_bytes(bytes);
    std::array::from_fn(|texel| index_at(bits, ALPHA_INDEX_BITS, texel))
}

/// Eight alpha values from 0 to 1: the endpoints plus either six or four interpolated steps
/// (the four-step form adds fully transparent and fully opaque).
fn alpha_ramp(first: u8, second: u8) -> [f32; 8] {
    let (first, second) = (unit_from_byte(first), unit_from_byte(second));
    if first > second {
        let [a, b, c, d, e, f] = alpha_steps(first, second, 7);
        [first, second, a, b, c, d, e, f]
    } else {
        let [a, b, c, d, _, _] = alpha_steps(first, second, 5);
        [first, second, a, b, c, d, 0.0, OPAQUE]
    }
}

/// The first six steps from `first` towards `second` in `intervals` equal parts.
// Derived from AuroraAsset.dll for interoperability: SSE `f32` weights.
fn alpha_steps(first: f32, second: f32, intervals: u8) -> [f32; 6] {
    let intervals = f32::from(intervals);
    std::array::from_fn(|step| {
        let step = step as f32 + 1.0;
        let weight_second = step / intervals;
        let weight_first = (intervals - step) / intervals;
        first * weight_first + second * weight_second
    })
}

/// Decodes the visible `width` x `height` pixels of a block-compressed surface laid out per
/// `layout`. The padding after the last visible block may be missing.
pub(super) fn decode_surface(
    data: &[u8],
    width: u32,
    height: u32,
    layout: &SurfaceLayout,
    codec: BlockCodec,
    endian: GpuEndian,
    channels: ChannelOrder,
) -> Result<RgbaImage, DecodeError> {
    let surface = layout.read_from(data)?;
    let (visible_width, visible_height) = (width as usize, height as usize);
    let mut image = RgbaImage::new(width, height);
    for block_y in 0..layout.units_high {
        for block_x in 0..layout.units_wide {
            let start = layout.unit_offset(block_x, block_y);
            let mut buffer = [0u8; BC3_BLOCK_BYTES];
            let block = &mut buffer[..codec.block_bytes()];
            block.copy_from_slice(&surface[start..start + block.len()]);
            endian.to_native(block);
            let texels = codec.decode_block(block);
            for (texel, values) in texels.iter().enumerate() {
                let x = block_x * BLOCK_SIDE + texel % BLOCK_SIDE;
                let y = block_y * BLOCK_SIDE + texel / BLOCK_SIDE;
                if x < visible_width && y < visible_height {
                    let pixel = Rgba(texel_to_bytes(channels.arrange(*values)));
                    image.put_pixel(x as u32, y as u32, pixel);
                }
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_with(color: u16, color_bits: u32, alpha: [u8; 2], alpha_bits: u64) -> [u8; 16] {
        let mut block = [0u8; BC3_BLOCK_BYTES];
        block[ALPHA_ENDPOINTS].copy_from_slice(&alpha);
        block[ALPHA_INDEX_BYTES]
            .copy_from_slice(&alpha_bits.to_le_bytes()[..ALPHA_INDEX_BYTES.len()]);
        let color_half = &mut block[COLOR_BLOCK_START..];
        color_half[COLOR_FIRST_ENDPOINT].copy_from_slice(&color.to_le_bytes());
        color_half[COLOR_SECOND_ENDPOINT].copy_from_slice(&color.to_le_bytes());
        color_half[COLOR_INDEX_BYTES].copy_from_slice(&color_bits.to_le_bytes());
        block
    }

    #[test]
    fn opaque_white_decodes_unchanged() {
        let block = block_with(0xFFFF, 0, [255, 255], 0);
        assert_eq!(decode_bc3_block(&block, true), [[255, 255, 255, 255]; 16]);
    }

    #[test]
    fn four_step_alpha_ramp_adds_transparent_and_opaque() {
        let ramp = alpha_ramp(10, 10);
        assert_eq!(ramp[6], 0.0);
        assert_eq!(ramp[7], 1.0);
    }

    #[test]
    fn six_step_alpha_ramp_interpolates_sevenths() {
        let ramp = alpha_ramp(7, 0).map(channel_to_byte);
        assert_eq!(ramp, [7, 0, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn bc1_uses_four_opaque_colors_when_first_endpoint_is_greater() {
        let block = [0xFF, 0xFF, 0x00, 0x00, 0b1110_0100, 0, 0, 0];
        let texels = decode_bc1_block(&block);
        assert_eq!(texels[0], [255, 255, 255, 255]);
        assert_eq!(texels[1], [0, 0, 0, 255]);
        assert_eq!(texels[2], [170, 170, 170, 255]);
        assert_eq!(texels[3], [85, 85, 85, 255]);
    }

    #[test]
    fn bc1_uses_midpoint_and_transparent_black_otherwise() {
        let block = [0x00, 0x00, 0xFF, 0xFF, 0b1110_0100, 0, 0, 0];
        let texels = decode_bc1_block(&block);
        assert_eq!(texels[0], [0, 0, 0, 255]);
        assert_eq!(texels[1], [255, 255, 255, 255]);
        assert_eq!(texels[2], [128, 128, 128, 255]);
        assert_eq!(texels[3], [0, 0, 0, 0]);
    }

    #[test]
    fn equal_endpoints_use_the_three_color_palette() {
        let block = [0x1F, 0x00, 0x1F, 0x00, 0b1110_0100, 0, 0, 0];
        assert_eq!(decode_bc1_block(&block)[3], [0, 0, 0, 0]);
    }

    #[test]
    fn bc3_color_half_uses_the_three_color_palette_without_transparency() {
        let mut block = block_with(0xFFFF, 0b11, [255, 255], 0);
        let color_half = &mut block[COLOR_BLOCK_START..];
        color_half[COLOR_FIRST_ENDPOINT].copy_from_slice(&0x0000u16.to_le_bytes());
        color_half[COLOR_SECOND_ENDPOINT].copy_from_slice(&0xFFFFu16.to_le_bytes());
        assert_eq!(decode_bc3_block(&block, false)[0], [0, 0, 0, 255]);
    }

    #[test]
    fn without_divide_color_and_alpha_are_rounded_directly() {
        let block = block_with(0xFFFF, 0, [128, 128], 0);
        assert_eq!(decode_bc3_block(&block, false)[0], [255, 255, 255, 128]);
    }
}
