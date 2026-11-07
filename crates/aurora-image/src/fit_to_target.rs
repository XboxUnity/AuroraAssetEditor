use image::RgbaImage;

use crate::image_target::ImageTarget;
use crate::resampler::RESIZE_FILTER;

/// Stretches to the exact target size, ignoring aspect ratio, as the old app did.
pub fn fit_to_target(image: RgbaImage, target: ImageTarget, auto_resize: bool) -> RgbaImage {
    let (width, height) = target.size();
    if !auto_resize || image.dimensions() == (width, height) {
        return image;
    }
    RESIZE_FILTER.resize(&image, width, height)
}
