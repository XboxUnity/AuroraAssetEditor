use image::RgbaImage;

use crate::gdiplus_alpha::{premultiply, unpremultiply};

/// The pixels the old app handed AuroraAsset.dll for `image`. `ImageToRawArgb` copied the image
/// with `new Bitmap(img)` before reading it, and GDI+ draws that copy through premultiplied alpha:
/// colour under alpha 0 becomes black and semi-transparent colour loses precision.
pub fn encoder_pixels(image: &RgbaImage) -> RgbaImage {
    let mut copy = image.clone();
    for pixel in copy.pixels_mut() {
        *pixel = unpremultiply(premultiply(*pixel));
    }
    copy
}
