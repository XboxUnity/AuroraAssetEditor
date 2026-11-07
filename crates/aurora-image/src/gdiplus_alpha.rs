//! GDI+'s conversions between straight and premultiplied 8-bit alpha, measured exactly against
//! `new Bitmap(img)` for every colour and alpha pair (docs/image/gdiplus-closeness.md).

use image::Rgba;

const OPAQUE: u32 = 255;
const ROUNDING_HALF: u32 = 128;
const BYTE_SHIFT: u32 = 8;
/// GDI+ divides by alpha with a 16.16 fixed-point reciprocal of 255 / alpha, truncated.
const RECIPROCAL_NUMERATOR: u32 = OPAQUE << 16;
const FIXED_POINT_SHIFT: u32 = 16;

/// `channel * alpha / 255`, rounded.
pub(crate) fn premultiply_channel(channel: u8, alpha: u8) -> u8 {
    let product = u32::from(channel) * u32::from(alpha) + ROUNDING_HALF;
    ((product + (product >> BYTE_SHIFT)) >> BYTE_SHIFT) as u8
}

pub(crate) fn unpremultiply_channel(premultiplied: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }
    let reciprocal = RECIPROCAL_NUMERATOR / u32::from(alpha);
    ((u32::from(premultiplied) * reciprocal) >> FIXED_POINT_SHIFT).min(OPAQUE) as u8
}

pub(crate) fn premultiply(pixel: Rgba<u8>) -> Rgba<u8> {
    let [red, green, blue, alpha] = pixel.0;
    Rgba([
        premultiply_channel(red, alpha),
        premultiply_channel(green, alpha),
        premultiply_channel(blue, alpha),
        alpha,
    ])
}

pub(crate) fn unpremultiply(pixel: Rgba<u8>) -> Rgba<u8> {
    let [red, green, blue, alpha] = pixel.0;
    Rgba([
        unpremultiply_channel(red, alpha),
        unpremultiply_channel(green, alpha),
        unpremultiply_channel(blue, alpha),
        alpha,
    ])
}
