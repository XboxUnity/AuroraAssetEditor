mod argb;
mod block_decode;
mod endian;
mod format;
mod header;
mod layout;

use std::fmt;

use image::RgbaImage;

use layout::SurfaceLayout;

pub use format::{Compression, TextureFormat};
pub use header::{TEXTURE_HEADER_LEN, TextureHeader};

pub const MAX_DIMENSION: u32 = 8192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    DimensionsOutOfRange { width: u32, height: u32 },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionsOutOfRange { width, height } => write!(
                f,
                "image size {width}x{height} is outside the supported range of 1 to {MAX_DIMENSION} pixels per side"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnsupportedFormat(TextureFormat),
    TruncatedData { expected: usize, actual: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(format) => {
                write!(f, "texture format {format:?} cannot be decoded")
            }
            Self::TruncatedData { expected, actual } => write!(
                f,
                "texture data is {actual} bytes but its header requires {expected}"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A texture as stored in an `.asset` entry: the 52-byte header plus the surface bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedTexture {
    pub header: TextureHeader,
    pub data: Vec<u8>,
}

pub fn encode(image: &RgbaImage, compression: Compression) -> Result<EncodedTexture, EncodeError> {
    let header = TextureHeader::for_image(image.width(), image.height(), compression)?;
    let layout = SurfaceLayout::for_compression(image.width(), image.height(), compression);
    let data = match compression {
        Compression::None => argb::pack(image, &layout),
        Compression::Dxt => todo!("DXT encode: Task 7"),
    };
    Ok(EncodedTexture { header, data })
}

/// The header's format and surface layout, once the data is known to be long enough.
fn checked_layout(texture: &EncodedTexture) -> Result<(TextureFormat, SurfaceLayout), DecodeError> {
    let header = &texture.header;
    let format = header.format();
    let layout = SurfaceLayout::for_header(header).ok_or(DecodeError::UnsupportedFormat(format))?;
    let expected = layout.padded_len();
    if texture.data.len() < expected {
        return Err(DecodeError::TruncatedData {
            expected,
            actual: texture.data.len(),
        });
    }
    Ok((format, layout))
}

pub fn decode(texture: &EncodedTexture) -> Result<RgbaImage, DecodeError> {
    let header = &texture.header;
    let (format, layout) = checked_layout(texture)?;
    match format {
        TextureFormat::LinearArgb8888 => Ok(argb::unpack(
            &texture.data,
            header.width(),
            header.height(),
            &layout,
        )),
        TextureFormat::LinearDxt4 => Ok(block_decode::decode_dxt4(
            &texture.data,
            header.width(),
            header.height(),
            &layout,
        )),
        TextureFormat::LinearDxt1 | TextureFormat::LinearDxt5 => {
            todo!("DXT1/DXT5 decode: Task 6")
        }
        TextureFormat::Unsupported { .. } => Err(DecodeError::UnsupportedFormat(format)),
    }
}

/// Pre-rounding RGBA channel values of a DXT4 decode, row-major. Exposed only so the golden
/// tests can prove the DLL differences are exact .5 rounding ties.
#[doc(hidden)]
pub fn dxt4_unrounded_values(texture: &EncodedTexture) -> Result<Vec<[f64; 4]>, DecodeError> {
    let (format, layout) = checked_layout(texture)?;
    if format != TextureFormat::LinearDxt4 {
        return Err(DecodeError::UnsupportedFormat(format));
    }
    let header = &texture.header;
    Ok(block_decode::dxt4_values(
        &texture.data,
        header.width(),
        header.height(),
        &layout,
    ))
}
