use std::error::Error;
use std::fmt;

use aurora_asset::EncodeError;
use aurora_image::ImportError;

/// Why an image of an FSD file could not be turned into a page entry. It reads as the error it wraps.
#[derive(Debug)]
pub enum ImageLoadError {
    Import(ImportError),
    Encode(EncodeError),
}

impl fmt::Display for ImageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageLoadError::Import(error) => error.fmt(f),
            ImageLoadError::Encode(error) => error.fmt(f),
        }
    }
}

impl Error for ImageLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImageLoadError::Import(error) => error.source(),
            ImageLoadError::Encode(_) => None,
        }
    }
}
