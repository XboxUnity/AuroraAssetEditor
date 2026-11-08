use image::RgbaImage;

use crate::resampler::Resampler;

/// Previews are only shown, never saved, so they take the smoothest filter rather than GDI+'s: Triangle
/// averages every source pixel under an output pixel when shrinking and blends linearly when enlarging,
/// without the overshoot of CatmullRom or Lanczos3, and premultiplied so the colour hidden under
/// transparent pixels does not bleed into the edges.
pub const PREVIEW_FILTER: Resampler = Resampler::TrianglePremultiplied;

/// The largest size with the aspect ratio of `source` that fits in `area`, at least 1 x 1. In the side
/// that limits it, the size equals the area's.
pub fn preview_size(source: (u32, u32), area: (u32, u32)) -> (u32, u32) {
    let (source_width, source_height) = (f64::from(source.0), f64::from(source.1));
    let scale = f64::min(
        f64::from(area.0) / source_width,
        f64::from(area.1) / source_height,
    );
    let fitted = |source_side: f64, area_side: u32| {
        ((source_side * scale).round() as u32).clamp(1, area_side.max(1))
    };
    (fitted(source_width, area.0), fitted(source_height, area.1))
}

/// `image` resized with `PREVIEW_FILTER` to fit in `area`, keeping its aspect ratio.
pub fn resize_preview(image: &RgbaImage, area: (u32, u32)) -> RgbaImage {
    let (width, height) = preview_size(image.dimensions(), area);
    PREVIEW_FILTER.resize(image, width, height)
}
