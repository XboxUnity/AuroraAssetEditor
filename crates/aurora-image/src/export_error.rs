use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ExportError {
    Encode(image::ImageError),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Encode(_) => write!(f, "the image could not be encoded"),
        }
    }
}

impl Error for ExportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ExportError::Encode(error) => Some(error),
        }
    }
}

impl From<image::ImageError> for ExportError {
    fn from(error: image::ImageError) -> Self {
        ExportError::Encode(error)
    }
}
