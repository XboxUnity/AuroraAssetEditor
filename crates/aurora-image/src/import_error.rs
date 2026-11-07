use std::error::Error;
use std::fmt;

use aurora_asset::dds::DdsError;

use crate::decode_image::{MAX_IMPORT_ALLOC, MAX_IMPORT_SIDE};

const BYTES_PER_MIB: u64 = 1024 * 1024;

#[derive(Debug)]
pub enum ImportError {
    UnknownFormat,
    TooLarge { width: u32, height: u32 },
    Decode(image::ImageError),
    Dds(DdsError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::UnknownFormat => {
                write!(f, "the file is not an image format the editor reads")
            }
            ImportError::TooLarge { width, height } => write!(
                f,
                "the image is {width}x{height} pixels, over the limit of {MAX_IMPORT_SIDE} pixels per side and {} MiB of pixels",
                MAX_IMPORT_ALLOC / BYTES_PER_MIB
            ),
            ImportError::Decode(_) => write!(f, "the image could not be decoded"),
            ImportError::Dds(_) => write!(f, "the DDS image could not be decoded"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImportError::UnknownFormat | ImportError::TooLarge { .. } => None,
            ImportError::Decode(error) => Some(error),
            ImportError::Dds(error) => Some(error),
        }
    }
}
