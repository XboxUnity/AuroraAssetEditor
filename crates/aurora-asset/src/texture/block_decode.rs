use image::{Rgba, RgbaImage};

use super::endian::swap_8in16;
use super::layout::SurfaceLayout;

const BLOCK_SIDE: usize = 4;
const TEXELS_PER_BLOCK: usize = BLOCK_SIDE * BLOCK_SIDE;
const BC3_BLOCK_BYTES: usize = 16;

const ALPHA_ENDPOINTS: std::ops::Range<usize> = 0..2;
const ALPHA_INDEX_BYTES: std::ops::Range<usize> = 2..8;
const COLOUR_BLOCK_START: usize = 8;
const ALPHA_INDEX_BITS: u32 = 3;
const COLOUR_INDEX_BITS: u32 = 2;
const COLOUR_INDEX_MASK: u32 = 0b11;
const ALPHA_INDEX_MASK: u64 = 0b111;

const RED_BITS_MAX: f64 = 31.0;
const GREEN_BITS_MAX: f64 = 63.0;
const BLUE_BITS_MAX: f64 = 31.0;
const CHANNEL_MAX: f64 = 255.0;

type Texel = [u8; 4];
type FloatColour = [f64; 3];

/// Which colour palette a BC1 block uses.
#[derive(Clone, Copy)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "DXT1 decode arrives with Task 6")
)]
pub(crate) enum Bc1Palette {
    /// Four opaque colours, as in the colour half of a BC3 block.
    FourColor,
    /// Three colours plus transparent black when the first endpoint is not greater than the second.
    #[cfg_attr(test, expect(dead_code, reason = "DXT1 decode arrives with Task 6"))]
    Dxt1,
}

/// Decodes a BC1 colour block (little-endian words) to RGBA texels, row-major within the block.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "DXT1 decode arrives with Task 6")
)]
pub(crate) fn decode_bc1_block(block: &[u8; 8], palette: Bc1Palette) -> [Texel; TEXELS_PER_BLOCK] {
    match palette {
        Bc1Palette::FourColor => {
            let colours = four_colour_palette(block);
            let indices = colour_indices(block);
            std::array::from_fn(|texel| {
                let [red, green, blue] = colours[indices[texel]];
                [
                    round_to_channel(red),
                    round_to_channel(green),
                    round_to_channel(blue),
                    u8::MAX,
                ]
            })
        }
        Bc1Palette::Dxt1 => todo!("DXT1 palette: Task 6"),
    }
}

/// Decodes a BC3 block to RGBA texels, row-major within the block.
///
/// With `divide_alpha`, colour is divided by alpha: the quirk of the DLL's DXT4 decode,
/// kept so that decoded pixels equal what Aurora's tooling shows.
pub(crate) fn decode_bc3_block(block: &[u8; 16], divide_alpha: bool) -> [Texel; TEXELS_PER_BLOCK] {
    let colour_block: [u8; 8] = std::array::from_fn(|offset| block[COLOUR_BLOCK_START + offset]);
    let colours = four_colour_palette(&colour_block);
    let colour_indices = colour_indices(&colour_block);
    let alphas = alpha_ramp(block[ALPHA_ENDPOINTS][0], block[ALPHA_ENDPOINTS][1]);
    let alpha_indices = alpha_indices(block);
    std::array::from_fn(|texel| {
        finish_texel(
            colours[colour_indices[texel]],
            alphas[alpha_indices[texel]],
            divide_alpha,
        )
    })
}

/// Final per-texel arithmetic, in `f64`. This is the single place to change when matching
/// the DLL's float precision exactly.
fn finish_texel(colour: FloatColour, alpha: f64, divide_alpha: bool) -> Texel {
    let colour = colour.map(|channel| {
        if !divide_alpha {
            channel
        } else if alpha > 0.0 {
            (channel * CHANNEL_MAX / alpha).min(CHANNEL_MAX)
        } else {
            0.0
        }
    });
    let [red, green, blue] = colour;
    [
        round_to_channel(red),
        round_to_channel(green),
        round_to_channel(blue),
        round_to_channel(alpha),
    ]
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
    let red = f64::from(packed >> 11 & 0b1_1111);
    let green = f64::from(packed >> 5 & 0b11_1111);
    let blue = f64::from(packed & 0b1_1111);
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

fn colour_indices(colour_block: &[u8; 8]) -> [usize; TEXELS_PER_BLOCK] {
    let bits = u32::from_le_bytes([
        colour_block[4],
        colour_block[5],
        colour_block[6],
        colour_block[7],
    ]);
    std::array::from_fn(|texel| {
        (bits >> (COLOUR_INDEX_BITS * texel as u32) & COLOUR_INDEX_MASK) as usize
    })
}

fn alpha_indices(block: &[u8; 16]) -> [usize; TEXELS_PER_BLOCK] {
    let mut bytes = [0u8; 8];
    bytes[..6].copy_from_slice(&block[ALPHA_INDEX_BYTES]);
    let bits = u64::from_le_bytes(bytes);
    std::array::from_fn(|texel| {
        (bits >> (ALPHA_INDEX_BITS * texel as u32) & ALPHA_INDEX_MASK) as usize
    })
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

/// Decodes the visible `width` x `height` pixels of a DXT4 surface laid out per `layout`.
/// `surface` must hold at least `layout.padded_len()` bytes.
pub(super) fn decode_dxt4(
    surface: &[u8],
    width: u32,
    height: u32,
    layout: &SurfaceLayout,
) -> RgbaImage {
    let mut unswapped = surface.to_vec();
    swap_8in16(&mut unswapped);
    let mut image = RgbaImage::new(width, height);
    for (block_y, row) in unswapped
        .chunks_exact(layout.row_pitch)
        .take((height as usize).div_ceil(BLOCK_SIDE))
        .enumerate()
    {
        let (blocks, _) = row.as_chunks::<BC3_BLOCK_BYTES>();
        for (block_x, block) in blocks
            .iter()
            .take((width as usize).div_ceil(BLOCK_SIDE))
            .enumerate()
        {
            let texels = decode_bc3_block(block, true);
            copy_visible_texels(&mut image, block_x, block_y, &texels);
        }
    }
    image
}

fn copy_visible_texels(
    image: &mut RgbaImage,
    block_x: usize,
    block_y: usize,
    texels: &[Texel; TEXELS_PER_BLOCK],
) {
    for (texel, rgba) in texels.iter().enumerate() {
        let x = block_x * BLOCK_SIDE + texel % BLOCK_SIDE;
        let y = block_y * BLOCK_SIDE + texel / BLOCK_SIDE;
        if x < image.width() as usize && y < image.height() as usize {
            image.put_pixel(x as u32, y as u32, Rgba(*rgba));
        }
    }
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
    fn bc1_four_colour_decode_is_opaque_and_ignores_endpoint_order() {
        let block = [0xFF, 0xFF, 0x00, 0x00, 0b1110_0100, 0, 0, 0];
        let texels = decode_bc1_block(&block, Bc1Palette::FourColor);
        assert_eq!(texels[0], [255, 255, 255, 255]);
        assert_eq!(texels[1], [0, 0, 0, 255]);
        assert_eq!(texels[2], [170, 170, 170, 255]);
        assert_eq!(texels[3], [85, 85, 85, 255]);
    }

    #[test]
    fn without_divide_colour_and_alpha_are_rounded_directly() {
        let block = block_with(0xFFFF, 0, [128, 128], 0);
        assert_eq!(decode_bc3_block(&block, false)[0], [255, 255, 255, 128]);
    }
}
