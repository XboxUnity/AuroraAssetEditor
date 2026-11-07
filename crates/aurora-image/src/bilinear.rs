use image::RgbaImage;

use crate::axis_taps::{AxisTaps, stretch};

/// Two-tap bilinear in straight alpha, pixel centres aligned, edges repeating the nearest pixel.
/// Unlike `image`'s Triangle filter it never widens its kernel when shrinking.
pub(crate) fn resize_bilinear(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let columns = AxisTaps::centred_bilinear(width, image.width());
    let rows = AxisTaps::centred_bilinear(height, image.height());
    stretch(image, &columns, &rows)
}
