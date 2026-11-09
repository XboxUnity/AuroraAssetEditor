use aurora_asset::RgbaImage;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub fn slint_image(pixels: &RgbaImage) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
    );
    Image::from_rgba8(buffer)
}
