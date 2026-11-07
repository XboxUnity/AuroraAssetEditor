use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

/// The filter used when stretching to a target size, until the GDI+ comparison picks one.
pub const RESIZE_FILTER: Resampler = Resampler::Triangle;

const OPAQUE: u32 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resampler {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
    TrianglePremultiplied,
    CatmullRomPremultiplied,
    Lanczos3Premultiplied,
}

impl Resampler {
    pub const ALL: [Resampler; 8] = [
        Resampler::Nearest,
        Resampler::Triangle,
        Resampler::CatmullRom,
        Resampler::Gaussian,
        Resampler::Lanczos3,
        Resampler::TrianglePremultiplied,
        Resampler::CatmullRomPremultiplied,
        Resampler::Lanczos3Premultiplied,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Resampler::Nearest => "Nearest",
            Resampler::Triangle => "Triangle",
            Resampler::CatmullRom => "CatmullRom",
            Resampler::Gaussian => "Gaussian",
            Resampler::Lanczos3 => "Lanczos3",
            Resampler::TrianglePremultiplied => "Triangle (premultiplied)",
            Resampler::CatmullRomPremultiplied => "CatmullRom (premultiplied)",
            Resampler::Lanczos3Premultiplied => "Lanczos3 (premultiplied)",
        }
    }

    pub fn resize(self, image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
        match self {
            Resampler::Nearest => imageops::resize(image, width, height, FilterType::Nearest),
            Resampler::Triangle => imageops::resize(image, width, height, FilterType::Triangle),
            Resampler::CatmullRom => imageops::resize(image, width, height, FilterType::CatmullRom),
            Resampler::Gaussian => imageops::resize(image, width, height, FilterType::Gaussian),
            Resampler::Lanczos3 => imageops::resize(image, width, height, FilterType::Lanczos3),
            Resampler::TrianglePremultiplied => {
                resize_premultiplied(image, width, height, FilterType::Triangle)
            }
            Resampler::CatmullRomPremultiplied => {
                resize_premultiplied(image, width, height, FilterType::CatmullRom)
            }
            Resampler::Lanczos3Premultiplied => {
                resize_premultiplied(image, width, height, FilterType::Lanczos3)
            }
        }
    }
}

fn resize_premultiplied(
    image: &RgbaImage,
    width: u32,
    height: u32,
    filter: FilterType,
) -> RgbaImage {
    let mut premultiplied = image.clone();
    premultiplied.pixels_mut().for_each(premultiply);
    let mut resized = imageops::resize(&premultiplied, width, height, filter);
    resized.pixels_mut().for_each(unpremultiply);
    resized
}

fn premultiply(pixel: &mut Rgba<u8>) {
    let alpha = u32::from(pixel[3]);
    for channel in &mut pixel.0[..3] {
        *channel = ((u32::from(*channel) * alpha + OPAQUE / 2) / OPAQUE) as u8;
    }
}

fn unpremultiply(pixel: &mut Rgba<u8>) {
    let alpha = u32::from(pixel[3]);
    for channel in &mut pixel.0[..3] {
        let scaled = u32::from(*channel) * OPAQUE + alpha / 2;
        *channel = scaled.checked_div(alpha).unwrap_or(0).min(OPAQUE) as u8;
    }
}
