use std::error::Error;
use std::fmt;
use std::io;

use aurora_asset::EncodeError;
use aurora_image::ImportError;

/// Why an image could not become a page entry. It reads as the error it wraps.
#[derive(Debug)]
pub enum ImportFailure {
    Read(io::Error),
    Decode(ImportError),
    Encode(EncodeError),
    /// The unused fourth entry of an asset file holds no picture.
    NoPicture,
}

impl fmt::Display for ImportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportFailure::Read(error) => error.fmt(f),
            ImportFailure::Decode(error) => error.fmt(f),
            ImportFailure::Encode(error) => error.fmt(f),
            ImportFailure::NoPicture => f.write_str("this entry of an asset file holds no picture"),
        }
    }
}

impl Error for ImportFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImportFailure::Read(error) => error.source(),
            ImportFailure::Decode(error) => error.source(),
            ImportFailure::Encode(_) | ImportFailure::NoPicture => None,
        }
    }
}
