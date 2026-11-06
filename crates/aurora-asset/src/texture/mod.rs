mod argb;
mod bc3_encode;
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
    TruncatedData {
        expected: usize,
        actual: usize,
    },
    RowPitchTooSmall {
        pitch_units: u32,
        required_units: u32,
    },
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
            Self::RowPitchTooSmall {
                pitch_units,
                required_units,
            } => write!(
                f,
                "texture row pitch of {pitch_units} units cannot hold a row, which needs {required_units}"
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
        Compression::Dxt => bc3_encode::encode_surface(image, &layout),
    };
    Ok(EncodedTexture { header, data })
}

/// The header's format and surface layout, once the data is known to be long enough.
fn checked_layout(texture: &EncodedTexture) -> Result<(TextureFormat, SurfaceLayout), DecodeError> {
    let header = &texture.header;
    let format = header.format();
    let layout = SurfaceLayout::for_header(header)?;
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
    if let Some(codec) = format.block_codec() {
        let channels = header
            .channel_order()
            .ok_or(DecodeError::UnsupportedFormat(format))?;
        return Ok(block_decode::decode_surface(
            &texture.data,
            header.width(),
            header.height(),
            &layout,
            codec,
            header.gpu_endian(),
            channels,
        ));
    }
    match format {
        TextureFormat::LinearArgb8888 => Ok(argb::unpack(
            &texture.data,
            header.width(),
            header.height(),
            &layout,
        )),
        _ => Err(DecodeError::UnsupportedFormat(format)),
    }
}

/// Pre-rounding RGBA channel values of a block-compressed decode, row-major. Exposed only so the
/// golden tests can prove the DLL differences are exact .5 rounding ties.
#[doc(hidden)]
pub fn unrounded_values(texture: &EncodedTexture) -> Result<Vec<[f64; 4]>, DecodeError> {
    let (format, layout) = checked_layout(texture)?;
    let header = &texture.header;
    let codec = format
        .block_codec()
        .ok_or(DecodeError::UnsupportedFormat(format))?;
    let channels = header
        .channel_order()
        .ok_or(DecodeError::UnsupportedFormat(format))?;
    Ok(block_decode::surface_values(
        &texture.data,
        header.width(),
        header.height(),
        &layout,
        codec,
        header.gpu_endian(),
        channels,
    ))
}
