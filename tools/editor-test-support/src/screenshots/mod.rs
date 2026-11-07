mod pixel_comparison;
mod png_file;

pub use pixel_comparison::{
    PixelBounds, PixelComparison, PixelDifference, changed_pixels, compare_pixels,
};
pub use png_file::{PngFileError, read_png, write_opaque_png};
