#![forbid(unsafe_code)]

mod axis_taps;
mod bilinear;
mod decode_image;
mod encoder_pixels;
mod export_error;
mod export_format;
mod fit_to_target;
mod gdiplus_alpha;
mod gdiplus_gray16_table;
mod gdiplus_resize;
mod image_target;
mod import_error;
mod resampler;

pub use decode_image::{IMPORT_EXTENSIONS, MAX_IMPORT_ALLOC, MAX_IMPORT_SIDE, decode_image};
pub use encoder_pixels::encoder_pixels;
pub use export_error::ExportError;
pub use export_format::ExportFormat;
pub use fit_to_target::fit_to_target;
pub use image_target::ImageTarget;
pub use import_error::ImportError;
pub use resampler::{RESIZE_FILTER, Resampler};
