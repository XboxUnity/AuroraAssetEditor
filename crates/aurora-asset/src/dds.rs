//! DDS decode as `AuroraAsset.dll`'s `ConvertDDSToImage` intends it: the top mip level of a DXT1,
//! DXT5, A8R8G8B8 or X8R8G8B8 file. See `docs/codec/xg-decode.md` for where the port deliberately
//! differs from the DLL.

use std::fmt;

use image::RgbaImage;

use crate::bytes::{BLOCK_SIDE, BYTES_PER_PIXEL, block_array, read_u32_le};
use crate::texture::{decode_bc1_block, decode_bc3_block};

const MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: u32 = 124;
const HEADER_SIZE_OFFSET: usize = 4;
const HEIGHT_OFFSET: usize = 12;
const WIDTH_OFFSET: usize = 16;
const PIXEL_FORMAT_FLAGS_OFFSET: usize = 80;
const FOURCC_OFFSET: usize = 84;
const BIT_COUNT_OFFSET: usize = 88;
const CHANNEL_MASKS_OFFSET: usize = 92;
const DATA_START: usize = MAGIC.len() + HEADER_SIZE as usize;

const PIXEL_FLAG_FOURCC: u32 = 0x4;
const PIXEL_FLAG_RGB: u32 = 0x40;
const FOURCC_DXT1: &[u8; 4] = b"DXT1";
const FOURCC_DXT5: &[u8; 4] = b"DXT5";

/// Red, green, blue and alpha masks of the 32-bit layouts, in header order.
const ARGB_MASKS: [u32; 4] = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000];
const XRGB_MASKS: [u32; 4] = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0];
const ARGB_BIT_COUNT: u32 = 32;

const DXT1_BLOCK_BYTES: usize = 8;
const DXT5_BLOCK_BYTES: usize = 16;
const OPAQUE: u8 = u8::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdsError {
    NotDds,
    Unsupported(String),
    Truncated,
}

impl fmt::Display for DdsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDds => write!(formatter, "the data is not a DDS file"),
            Self::Unsupported(reason) => write!(formatter, "the DDS file is unsupported: {reason}"),
            Self::Truncated => write!(formatter, "the DDS file is shorter than its header says"),
        }
    }
}

impl std::error::Error for DdsError {}

enum Format {
    Dxt1,
    Dxt5,
    Argb8888,
    Xrgb8888,
}

type Texels = [[u8; 4]; BLOCK_SIDE * BLOCK_SIDE];

pub fn decode(bytes: &[u8]) -> Result<RgbaImage, DdsError> {
    if !bytes.starts_with(MAGIC) {
        return Err(DdsError::NotDds);
    }
    if bytes.len() < DATA_START {
        return Err(DdsError::Truncated);
    }
    if read_u32_le(bytes, HEADER_SIZE_OFFSET) != HEADER_SIZE {
        return Err(unsupported("the header size is not 124"));
    }
    let width = read_u32_le(bytes, WIDTH_OFFSET);
    let height = read_u32_le(bytes, HEIGHT_OFFSET);
    if width == 0 || height == 0 {
        return Err(unsupported("the image has no pixels"));
    }
    let pixels = &bytes[DATA_START..];
    let rgba = match format_of(bytes)? {
        Format::Dxt1 => decode_blocks(pixels, width, height, DXT1_BLOCK_BYTES, |block| {
            decode_bc1_block(&block_array(block))
        })?,
        Format::Dxt5 => decode_blocks(pixels, width, height, DXT5_BLOCK_BYTES, |block| {
            decode_bc3_block(&block_array(block), false)
        })?,
        Format::Argb8888 => decode_bgra(pixels, width, height, Alpha::Stored)?,
        Format::Xrgb8888 => decode_bgra(pixels, width, height, Alpha::Opaque)?,
    };
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| unsupported("the image is too large"))
}

fn unsupported(reason: &str) -> DdsError {
    DdsError::Unsupported(reason.to_string())
}

fn format_of(bytes: &[u8]) -> Result<Format, DdsError> {
    let flags = read_u32_le(bytes, PIXEL_FORMAT_FLAGS_OFFSET);
    if flags & PIXEL_FLAG_FOURCC != 0 {
        return match &bytes[FOURCC_OFFSET..FOURCC_OFFSET + 4] {
            fourcc if fourcc == FOURCC_DXT1 => Ok(Format::Dxt1),
            fourcc if fourcc == FOURCC_DXT5 => Ok(Format::Dxt5),
            _ => Err(unsupported("only DXT1 and DXT5 compression decode")),
        };
    }
    let masks: [u32; 4] =
        std::array::from_fn(|channel| read_u32_le(bytes, CHANNEL_MASKS_OFFSET + 4 * channel));
    let is_32_bit_rgb =
        flags & PIXEL_FLAG_RGB != 0 && read_u32_le(bytes, BIT_COUNT_OFFSET) == ARGB_BIT_COUNT;
    match masks {
        ARGB_MASKS if is_32_bit_rgb => Ok(Format::Argb8888),
        XRGB_MASKS if is_32_bit_rgb => Ok(Format::Xrgb8888),
        _ => Err(unsupported(
            "only A8R8G8B8 and X8R8G8B8 uncompressed layouts decode",
        )),
    }
}

fn rgba_len(width: u32, height: u32) -> Result<usize, DdsError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
        .ok_or_else(|| unsupported("the image is too large"))
}

enum Alpha {
    Stored,
    /// The DLL keeps the unused byte of X8R8G8B8 as alpha; the port makes the pixels opaque so
    /// that an imported image is not transparent in Aurora.
    Opaque,
}

/// Pixels stored as little-endian ARGB words, that is B, G, R, A bytes.
fn decode_bgra(pixels: &[u8], width: u32, height: u32, alpha: Alpha) -> Result<Vec<u8>, DdsError> {
    let len = rgba_len(width, height)?;
    let stored = pixels.get(..len).ok_or(DdsError::Truncated)?;
    let mut rgba = Vec::with_capacity(len);
    for &[blue, green, red, stored_alpha] in stored.as_chunks::<BYTES_PER_PIXEL>().0 {
        let alpha = match alpha {
            Alpha::Stored => stored_alpha,
            Alpha::Opaque => OPAQUE,
        };
        rgba.extend_from_slice(&[red, green, blue, alpha]);
    }
    Ok(rgba)
}

/// Decodes 4x4 blocks stored row by row, dropping the texels of edge blocks that fall outside the
/// image.
// The DLL writes the partial block column of an image whose width is not a multiple of 4 into the
// next row; the port crops instead (see `docs/codec/xg-decode.md`).
fn decode_blocks(
    blocks: &[u8],
    width: u32,
    height: u32,
    block_bytes: usize,
    decode_block: impl Fn(&[u8]) -> Texels,
) -> Result<Vec<u8>, DdsError> {
    let (width, height) = (width as usize, height as usize);
    let blocks_across = width.div_ceil(BLOCK_SIDE);
    let blocks_down = height.div_ceil(BLOCK_SIDE);
    let needed = blocks_across
        .checked_mul(blocks_down)
        .and_then(|count| count.checked_mul(block_bytes))
        .ok_or_else(|| unsupported("the image is too large"))?;
    let blocks = blocks.get(..needed).ok_or(DdsError::Truncated)?;
    let mut rgba = vec![0; rgba_len(width as u32, height as u32)?];
    for (index, block) in blocks.chunks_exact(block_bytes).enumerate() {
        let origin_x = index % blocks_across * BLOCK_SIDE;
        let origin_y = index / blocks_across * BLOCK_SIDE;
        for (texel, colour) in decode_block(block).iter().enumerate() {
            let x = origin_x + texel % BLOCK_SIDE;
            let y = origin_y + texel / BLOCK_SIDE;
            if x < width && y < height {
                let start = (y * width + x) * BYTES_PER_PIXEL;
                rgba[start..start + BYTES_PER_PIXEL].copy_from_slice(colour);
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DXT1 file of `width` x `height` whose block `n` has both endpoints equal to `n` as a
    /// 565 colour and all indices 0, so each block decodes to one flat colour.
    fn numbered_dxt1(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        bytes.resize(DATA_START, 0);
        bytes[HEADER_SIZE_OFFSET..HEADER_SIZE_OFFSET + 4]
            .copy_from_slice(&HEADER_SIZE.to_le_bytes());
        bytes[HEIGHT_OFFSET..HEIGHT_OFFSET + 4].copy_from_slice(&height.to_le_bytes());
        bytes[WIDTH_OFFSET..WIDTH_OFFSET + 4].copy_from_slice(&width.to_le_bytes());
        bytes[PIXEL_FORMAT_FLAGS_OFFSET..PIXEL_FORMAT_FLAGS_OFFSET + 4]
            .copy_from_slice(&PIXEL_FLAG_FOURCC.to_le_bytes());
        bytes[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(FOURCC_DXT1);
        let block_count = width.div_ceil(4) * height.div_ceil(4);
        for number in 0..block_count as u16 {
            let colour = (number + 1) << 11;
            bytes.extend(colour.to_le_bytes());
            bytes.extend(colour.to_le_bytes());
            bytes.extend([0; 4]);
        }
        bytes
    }

    #[test]
    fn partial_edge_blocks_are_cropped_in_place() {
        let (width, height) = (18, 10);
        let image = decode(&numbered_dxt1(width, height)).unwrap();
        let blocks_across = width.div_ceil(4);
        let block_bytes = |n: u32| {
            let colour = (n as u16 + 1) << 11;
            let mut block = [0u8; 8];
            block[..2].copy_from_slice(&colour.to_le_bytes());
            block[2..4].copy_from_slice(&colour.to_le_bytes());
            block
        };
        for y in 0..height {
            for x in 0..width {
                let block = y / 4 * blocks_across + x / 4;
                let texel = (y % 4 * 4 + x % 4) as usize;
                let expected = decode_bc1_block(&block_bytes(block))[texel];
                assert_eq!(image.get_pixel(x, y).0, expected, "pixel ({x}, {y})");
            }
        }
    }
}
