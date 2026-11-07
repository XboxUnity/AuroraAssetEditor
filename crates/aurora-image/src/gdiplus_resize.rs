use image::RgbaImage;

use crate::axis_taps::{AxisTaps, stretch};
use crate::gdiplus_alpha::{premultiply, unpremultiply};

/// From this many pixels on either side GDI+ averages areas along every axis it shrinks.
const GDIPLUS_AREA_AVERAGING_SIDE: u32 = 420;

fn gdiplus_axis(output_len: u32, source_len: u32, averages_areas: bool) -> AxisTaps {
    if averages_areas && output_len < source_len {
        AxisTaps::gdiplus_area(output_len, source_len)
    } else {
        AxisTaps::gdiplus_bilinear(output_len, source_len)
    }
}

/// What `new Bitmap(img, size)` does with GDI+'s default `DrawImage` settings, as measured
/// (docs/image/gdiplus-closeness.md): it resamples premultiplied pixels, x before y, bilinearly
/// with pixel origins aligned and transparent black past the right and bottom edges, except that
/// a source of 420 pixels or more on either side has every shrinking axis averaged by area.
pub(crate) fn resize_gdiplus(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let averages_areas = image.width().max(image.height()) >= GDIPLUS_AREA_AVERAGING_SIDE;
    let columns = gdiplus_axis(width, image.width(), averages_areas);
    let rows = gdiplus_axis(height, image.height(), averages_areas);
    let mut premultiplied = image.clone();
    for pixel in premultiplied.pixels_mut() {
        *pixel = premultiply(*pixel);
    }
    let mut resized = stretch(&premultiplied, &columns, &rows);
    for pixel in resized.pixels_mut() {
        *pixel = unpremultiply(*pixel);
    }
    resized
}
