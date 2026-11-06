mod endian;
mod format;
mod header;
mod layout;

use std::fmt;

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
