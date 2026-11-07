use std::fmt;

use super::MAX_DIMENSION;

/// Why an image cannot be encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    DimensionsOutOfRange { width: u32, height: u32 },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionsOutOfRange { width, height } => write!(
                f,
                "image size {width}x{height} is outside the supported range of 1 to \
                 {MAX_DIMENSION} pixels per side"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}
