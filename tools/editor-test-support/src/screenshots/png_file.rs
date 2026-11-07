use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, ImageReader, Rgb, RgbImage, RgbaImage};

/// Writes `image` as an RGB8 PNG, as small as the encoder can make it, because references live
/// in git. Refuses an image with any translucent pixel, which RGB cannot hold.
pub fn write_opaque_png(path: &Path, image: &RgbaImage) -> Result<(), PngFileError> {
    if let Some((x, y)) = first_translucent_pixel(image) {
        return Err(PngFileError::NotOpaque { x, y });
    }
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }

    let rgb = RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [red, green, blue, _] = image.get_pixel(x, y).0;
        Rgb([red, green, blue])
    });
    let writer = BufWriter::new(File::create(path)?);
    PngEncoder::new_with_quality(writer, CompressionType::Best, FilterType::Adaptive).write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(())
}

/// Reads any PNG and converts it to RGBA8.
pub fn read_png(path: &Path) -> Result<RgbaImage, PngFileError> {
    Ok(ImageReader::open(path)?.decode()?.to_rgba8())
}

/// Row by row, the first pixel whose alpha is below 255.
fn first_translucent_pixel(image: &RgbaImage) -> Option<(u32, u32)> {
    image
        .enumerate_pixels()
        .find(|(_, _, pixel)| pixel.0[3] < u8::MAX)
        .map(|(x, y, _)| (x, y))
}

#[derive(Debug)]
pub enum PngFileError {
    NotOpaque { x: u32, y: u32 },
    Io(io::Error),
    Image(image::ImageError),
}

impl fmt::Display for PngFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOpaque { x, y } => {
                write!(formatter, "the pixel at ({x}, {y}) is not fully opaque")
            }
            Self::Io(error) => write!(formatter, "the PNG file could not be accessed: {error}"),
            Self::Image(error) => write!(formatter, "the PNG could not be coded: {error}"),
        }
    }
}

impl Error for PngFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotOpaque { .. } => None,
            Self::Io(error) => Some(error),
            Self::Image(error) => Some(error),
        }
    }
}

impl From<io::Error> for PngFileError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<image::ImageError> for PngFileError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error)
    }
}
