#![forbid(unsafe_code)]

mod decode_image;
mod import_error;

pub use decode_image::{IMPORT_EXTENSIONS, MAX_IMPORT_ALLOC, MAX_IMPORT_SIDE, decode_image};
pub use import_error::ImportError;
