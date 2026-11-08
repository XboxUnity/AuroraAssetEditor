use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Rgb, RgbImage, RgbaImage};

const TEMPORARY_SUFFIX: &str = ".partial";

/// Writes `image` as an RGB8 PNG, as small as the encoder can make it: every run's images go into
/// the release zip and the README images into git. Refuses an image with any translucent pixel,
/// which RGB cannot hold. The file is written as `write_replacing` writes it.
pub fn write_opaque_png(path: &Path, image: &RgbaImage) -> Result<(), PngFileError> {
    let bytes = opaque_png_bytes(image)?;
    write_replacing(path, &bytes)
}

/// The PNG `write_opaque_png` writes, in memory.
pub(super) fn opaque_png_bytes(image: &RgbaImage) -> Result<Vec<u8>, PngFileError> {
    if let Some((x, y)) = first_translucent_pixel(image) {
        return Err(PngFileError::NotOpaque { x, y });
    }
    encode_rgb_png(image)
}

/// Moves `bytes` into place at `path` from a temporary file next to it, so a failure never leaves a
/// half-written file and never changes a file already there.
pub(super) fn write_replacing(path: &Path, bytes: &[u8]) -> Result<(), PngFileError> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let temporary_path = temporary_path_beside(path);
    if let Err(error) = fs::write(&temporary_path, bytes) {
        return Err(remove_temporary_file(&temporary_path, error));
    }
    if let Err(error) = fs::rename(&temporary_path, path) {
        return Err(remove_temporary_file(&temporary_path, error));
    }
    Ok(())
}

/// Reads a PNG and converts it to RGBA8. The format is fixed, not guessed from the extension.
pub fn read_png(path: &Path) -> Result<RgbaImage, PngFileError> {
    let file = BufReader::new(File::open(path)?);
    let image = ImageReader::with_format(file, ImageFormat::Png)
        .decode()
        .map_err(PngFileError::Decode)?;
    Ok(image.to_rgba8())
}

fn encode_rgb_png(image: &RgbaImage) -> Result<Vec<u8>, PngFileError> {
    let rgb = RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [red, green, blue, _] = image.get_pixel(x, y).0;
        Rgb([red, green, blue])
    });
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::Adaptive)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(PngFileError::Encode)?;
    Ok(bytes)
}

fn temporary_path_beside(path: &Path) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(TEMPORARY_SUFFIX);
    PathBuf::from(name)
}

/// Removes the temporary file after a failed write and returns the failure to report. If the
/// removal fails as well, that is reported too, so no stray file goes unnoticed.
fn remove_temporary_file(temporary_path: &Path, failure: io::Error) -> PngFileError {
    match fs::remove_file(temporary_path) {
        Ok(()) => PngFileError::Io(failure),
        Err(removal) if removal.kind() == io::ErrorKind::NotFound => PngFileError::Io(failure),
        Err(removal) => PngFileError::Io(io::Error::new(
            failure.kind(),
            format!("{failure}; the temporary file could not be removed either: {removal}"),
        )),
    }
}

/// Row by row, the first pixel whose alpha is below 255.
pub(super) fn first_translucent_pixel(image: &RgbaImage) -> Option<(u32, u32)> {
    image
        .enumerate_pixels()
        .find(|(_, _, pixel)| pixel.0[3] < u8::MAX)
        .map(|(x, y, _)| (x, y))
}

#[derive(Debug)]
pub enum PngFileError {
    NotOpaque {
        x: u32,
        y: u32,
    },
    Io(io::Error),
    /// The file is not a PNG the decoder can read.
    Decode(image::ImageError),
    /// The image could not be encoded as a PNG.
    Encode(image::ImageError),
}

impl fmt::Display for PngFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOpaque { x, y } => {
                write!(formatter, "the pixel at ({x}, {y}) is not fully opaque")
            }
            Self::Io(error) => write!(formatter, "the PNG file could not be accessed: {error}"),
            Self::Decode(error) => write!(formatter, "the file is not a valid PNG: {error}"),
            Self::Encode(error) => write!(formatter, "the image could not be encoded: {error}"),
        }
    }
}

impl Error for PngFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotOpaque { .. } => None,
            Self::Io(error) => Some(error),
            Self::Decode(error) | Self::Encode(error) => Some(error),
        }
    }
}

impl From<io::Error> for PngFileError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
