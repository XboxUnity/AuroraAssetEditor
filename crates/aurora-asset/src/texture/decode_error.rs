use std::fmt;

use super::texture_format::TextureFormat;

/// Why a texture cannot be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The header describes a format the port does not decode.
    UnsupportedFormat(TextureFormat),
    /// The data ends before the last byte the decode reads.
    TruncatedData { expected: usize, actual: usize },
    /// The header's row pitch is narrower than one row of the image.
    RowPitchTooSmall {
        pitch_units: u32,
        required_units: u32,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(format) => {
                write!(f, "a texture in {format} cannot be decoded")
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
                "texture row pitch of {pitch_units} units cannot hold a row, which needs \
                 {required_units}"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}
