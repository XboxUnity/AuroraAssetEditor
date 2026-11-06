use image::{Rgba, RgbaImage};

use super::endian::GpuEndian;
use super::format::ChannelOrder;
use super::layout::SurfaceLayout;

const BLOCK_SIDE: usize = 4;
const TEXELS_PER_BLOCK: usize = BLOCK_SIDE * BLOCK_SIDE;
const BC1_BLOCK_BYTES: usize = 8;
const BC3_BLOCK_BYTES: usize = 16;

const ALPHA_ENDPOINTS: std::ops::Range<usize> = 0..2;
const ALPHA_INDEX_BYTES: std::ops::Range<usize> = 2..8;
const COLOUR_BLOCK_START: usize = 8;
const ALPHA_INDEX_BITS: u32 = 3;
const COLOUR_INDEX_BITS: u32 = 2;

const RED_SHIFT: u32 = 11;
const GREEN_SHIFT: u32 = 5;
const RED_BLUE_MASK_5_BITS: u64 = 0b1_1111;
const GREEN_MASK_6_BITS: u64 = 0b11_1111;

const RED_BITS_MAX: f64 = 31.0;
const GREEN_BITS_MAX: f64 = 63.0;
const BLUE_BITS_MAX: f64 = 31.0;
const CHANNEL_MAX: f64 = 255.0;

type Texel = [u8; 4];
type FloatColour = [f64; 3];
type FloatTexel = [f64; 4];

const TRANSPARENT_BLACK: FloatTexel = [0.0, 0.0, 0.0, 0.0];

/// How the blocks of a surface are decoded.
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

fn block_array<const BYTES: usize>(block: &[u8]) -> [u8; BYTES] {
    std::array::from_fn(|offset| block[offset])
}

/// Decodes a BC1 colour block (little-endian words) to RGBA texels, row-major within the block.
#[cfg(test)]
pub(crate) fn decode_bc1_block(block: &[u8; 8]) -> [Texel; TEXELS_PER_BLOCK] {
    decode_bc1_block_values(block).map(round_texel)
}

fn decode_bc1_block_values(block: &[u8; 8]) -> [FloatTexel; TEXELS_PER_BLOCK] {
    let colours = dxt1_palette(block);
    let indices = colour_indices(block);
    std::array::from_fn(|texel| colours[indices[texel]])
}

fn opaque([red, green, blue]: FloatColour) -> FloatTexel {
    [red, green, blue, CHANNEL_MAX]
}

/// The palette of a BC1 block and of the colour half of a BC3 block: four colours when the first endpoint is greater than the second, otherwise
/// the two endpoints, their midpoint and transparent black.
fn dxt1_palette(block: &[u8; 8]) -> [FloatTexel; 4] {
    let first_packed = u16::from_le_bytes([block[0], block[1]]);
    let second_packed = u16::from_le_bytes([block[2], block[3]]);
    if first_packed > second_packed {
        return four_colour_palette(block).map(opaque);
    }
    let first = expand_565(first_packed);
    let second = expand_565(second_packed);
    let midpoint = std::array::from_fn(|channel| (first[channel] + second[channel]) / 2.0);
    [
        opaque(first),
        opaque(second),
        opaque(midpoint),
        TRANSPARENT_BLACK,
    ]
}

/// Decodes a BC3 block to RGBA texels, row-major within the block.
#[cfg(test)]
pub(crate) fn decode_bc3_block(block: &[u8; 16], divide_alpha: bool) -> [Texel; TEXELS_PER_BLOCK] {
    decode_bc3_block_values(block, divide_alpha).map(round_texel)
}

fn decode_bc3_block_values(block: &[u8; 16], divide_alpha: bool) -> [FloatTexel; TEXELS_PER_BLOCK] {
    let colour_block: [u8; 8] = std::array::from_fn(|offset| block[COLOUR_BLOCK_START + offset]);
    let colours = dxt1_palette(&colour_block).map(|[red, green, blue, _]| [red, green, blue]);
    let colour_indices = colour_indices(&colour_block);
    let alphas = alpha_ramp(block[ALPHA_ENDPOINTS][0], block[ALPHA_ENDPOINTS][1]);
    let alpha_indices = alpha_indices(block);
    std::array::from_fn(|texel| {
        texel_values(
            colours[colour_indices[texel]],
            alphas[alpha_indices[texel]],
            divide_alpha,
        )
    })
}

/// Per-texel arithmetic before rounding, in `f64`: the alpha divide and clamp. This is the single
/// place to change when matching the DLL's float precision exactly.
fn texel_values(colour: FloatColour, alpha: f64, divide_alpha: bool) -> FloatTexel {
    let [red, green, blue] = colour.map(|channel| {
        if !divide_alpha {
            channel
        } else if alpha > 0.0 {
            (channel * CHANNEL_MAX / alpha).min(CHANNEL_MAX)
        } else {
            0.0
        }
    });
    [red, green, blue, alpha]
}

fn round_texel(values: FloatTexel) -> Texel {
    values.map(round_to_channel)
}

fn round_to_channel(value: f64) -> u8 {
    value.round_ties_even().clamp(0.0, CHANNEL_MAX) as u8
}

fn four_colour_palette(colour_block: &[u8; 8]) -> [FloatColour; 4] {
    let first = expand_565(u16::from_le_bytes([colour_block[0], colour_block[1]]));
    let second = expand_565(u16::from_le_bytes([colour_block[2], colour_block[3]]));
    [
        first,
        second,
        blend(first, second, 2.0, 1.0),
        blend(first, second, 1.0, 2.0),
    ]
}

fn expand_565(packed: u16) -> FloatColour {
    let packed = u64::from(packed);
    let red = bit_field(packed, RED_SHIFT, RED_BLUE_MASK_5_BITS) as f64;
    let green = bit_field(packed, GREEN_SHIFT, GREEN_MASK_6_BITS) as f64;
    let blue = bit_field(packed, 0, RED_BLUE_MASK_5_BITS) as f64;
    [
        red * CHANNEL_MAX / RED_BITS_MAX,
        green * CHANNEL_MAX / GREEN_BITS_MAX,
        blue * CHANNEL_MAX / BLUE_BITS_MAX,
    ]
}

/// `(weight_first * first + weight_second * second) / 3` per channel.
fn blend(
    first: FloatColour,
    second: FloatColour,
    weight_first: f64,
    weight_second: f64,
) -> FloatColour {
    std::array::from_fn(|channel| {
        (weight_first * first[channel] + weight_second * second[channel]) / 3.0
    })
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

fn colour_indices(colour_block: &[u8; 8]) -> [usize; TEXELS_PER_BLOCK] {
    let bits = u32::from_le_bytes([
        colour_block[4],
        colour_block[5],
        colour_block[6],
        colour_block[7],
    ]);
    std::array::from_fn(|texel| index_at(u64::from(bits), COLOUR_INDEX_BITS, texel))
}

fn alpha_indices(block: &[u8; 16]) -> [usize; TEXELS_PER_BLOCK] {
    let mut bytes = [0u8; 8];
    bytes[..6].copy_from_slice(&block[ALPHA_INDEX_BYTES]);
    let bits = u64::from_le_bytes(bytes);
    std::array::from_fn(|texel| index_at(bits, ALPHA_INDEX_BITS, texel))
}

/// Eight alpha values: the endpoints plus either six or four interpolated steps
/// (the four-step form adds fully transparent and fully opaque).
fn alpha_ramp(first: u8, second: u8) -> [f64; 8] {
    let (first, second) = (f64::from(first), f64::from(second));
    let steps = |count: u32| -> [f64; 6] {
        let total = f64::from(count) + 1.0;
        std::array::from_fn(|step| {
            let step = step as f64 + 1.0;
            ((total - step) * first + step * second) / total
        })
    };
    if first > second {
        let [a, b, c, d, e, f] = steps(6);
        [first, second, a, b, c, d, e, f]
    } else {
        let [a, b, c, d, _, _] = steps(4);
        [first, second, a, b, c, d, 0.0, CHANNEL_MAX]
    }
}

/// Decodes the visible `width` x `height` pixels of a block-compressed surface laid out per
/// `layout`. `surface` must hold at least `layout.padded_len()` bytes.
pub(super) fn decode_surface(
    surface: &[u8],
    width: u32,
    height: u32,
    layout: &SurfaceLayout,
    codec: BlockCodec,
    endian: GpuEndian,
    channels: ChannelOrder,
) -> RgbaImage {
    let values = surface_values(surface, width, height, layout, codec, endian, channels);
    RgbaImage::from_fn(width, height, |x, y| {
        Rgba(round_texel(
            values[y as usize * width as usize + x as usize],
        ))
    })
}

/// Channel values before rounding for every visible pixel, row-major.
pub(super) fn surface_values(
    surface: &[u8],
    width: u32,
    height: u32,
    layout: &SurfaceLayout,
    codec: BlockCodec,
    endian: GpuEndian,
    channels: ChannelOrder,
) -> Vec<FloatTexel> {
    let (width, height) = (width as usize, height as usize);
    let mut unswapped = surface.to_vec();
    endian.to_native(&mut unswapped);
    let mut values = vec![[0.0; 4]; width * height];
    for (block_y, row) in unswapped
        .chunks_exact(layout.row_pitch)
        .take(height.div_ceil(BLOCK_SIDE))
        .enumerate()
    {
        let blocks = row
            .chunks_exact(codec.block_bytes())
            .take(width.div_ceil(BLOCK_SIDE));
        for (block_x, block) in blocks.enumerate() {
            let texels = codec.decode_block(block);
            for (texel, texel_values) in texels.iter().enumerate() {
                let x = block_x * BLOCK_SIDE + texel % BLOCK_SIDE;
                let y = block_y * BLOCK_SIDE + texel / BLOCK_SIDE;
                if x < width && y < height {
                    values[y * width + x] = channels.arrange(*texel_values);
                }
            }
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_with(colour: u16, colour_bits: u32, alpha: [u8; 2], alpha_bits: u64) -> [u8; 16] {
        let mut block = [0u8; 16];
        block[0] = alpha[0];
        block[1] = alpha[1];
        block[2..8].copy_from_slice(&alpha_bits.to_le_bytes()[..6]);
        block[8..10].copy_from_slice(&colour.to_le_bytes());
        block[10..12].copy_from_slice(&colour.to_le_bytes());
        block[12..16].copy_from_slice(&colour_bits.to_le_bytes());
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
        assert_eq!(ramp[7], 255.0);
    }

    #[test]
    fn six_step_alpha_ramp_interpolates_sevenths() {
        let ramp = alpha_ramp(7, 0);
        assert_eq!(ramp, [7.0, 0.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
    }

    #[test]
    fn bc1_uses_four_opaque_colours_when_first_endpoint_is_greater() {
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
    fn equal_endpoints_use_the_three_colour_palette() {
        let block = [0x1F, 0x00, 0x1F, 0x00, 0b1110_0100, 0, 0, 0];
        assert_eq!(decode_bc1_block(&block)[3], [0, 0, 0, 0]);
    }

    #[test]
    fn bc3_colour_half_uses_the_three_colour_palette_without_transparency() {
        let mut block = block_with(0xFFFF, 0b11, [255, 255], 0);
        block[8..12].copy_from_slice(&[0x00, 0x00, 0xFF, 0xFF]);
        assert_eq!(decode_bc3_block(&block, false)[0], [0, 0, 0, 255]);
    }

    #[test]
    fn without_divide_colour_and_alpha_are_rounded_directly() {
        let block = block_with(0xFFFF, 0, [128, 128], 0);
        assert_eq!(decode_bc3_block(&block, false)[0], [255, 255, 255, 128]);
    }
}
