use std::io::Cursor;

use aurora_asset::RgbaImage;
use aurora_asset::dds;
use image::{ImageError, ImageFormat, ImageReader, Limits};

use crate::import_error::ImportError;

pub const IMPORT_EXTENSIONS: [&str; 12] = [
    "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp", "tga", "ico", "qoi", "dds",
];
pub const MAX_IMPORT_SIDE: u32 = 16_384;
// Bounds the decoder buffer. The RGBA copy comes on top and can add up to 4x of it
// (peak about 0.9 GiB for an 11585x11585 RGB image).
pub const MAX_IMPORT_ALLOC: u64 = 512 * 1024 * 1024;

const DDS_MAGIC: &[u8] = b"DDS ";
const TGA_EXTENSION: &str = "tga";
const RGBA_BYTES_PER_PIXEL: u64 = 4;

/// Decodes any import format to straight-alpha RGBA8. `file_name` only matters for TGA,
/// which has no magic number.
pub fn decode_image(bytes: &[u8], file_name: Option<&str>) -> Result<RgbaImage, ImportError> {
    if bytes.starts_with(DDS_MAGIC) {
        return dds::decode(bytes).map_err(ImportError::Dds);
    }

    let format = detect_format(bytes, file_name)?;
    let (width, height) = read_dimensions(bytes, format)?;
    check_size(width, height)?;

    let mut reader = reader_for(bytes, format);
    reader.limits(import_limits());
    match reader.decode() {
        Ok(image) => Ok(image.into_rgba8()),
        Err(ImageError::Limits(_)) => Err(ImportError::TooLarge { width, height }),
        Err(error) => Err(ImportError::Decode(error)),
    }
}

fn detect_format(bytes: &[u8], file_name: Option<&str>) -> Result<ImageFormat, ImportError> {
    if let Ok(format) = image::guess_format(bytes) {
        return Ok(format);
    }
    if file_name.is_some_and(has_tga_extension) {
        return Ok(ImageFormat::Tga);
    }
    Err(ImportError::UnknownFormat)
}

fn has_tga_extension(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(TGA_EXTENSION))
}

fn reader_for(bytes: &[u8], format: ImageFormat) -> ImageReader<Cursor<&[u8]>> {
    ImageReader::with_format(Cursor::new(bytes), format)
}

fn read_dimensions(bytes: &[u8], format: ImageFormat) -> Result<(u32, u32), ImportError> {
    reader_for(bytes, format)
        .into_dimensions()
        .map_err(ImportError::Decode)
}

fn check_size(width: u32, height: u32) -> Result<(), ImportError> {
    let allocation = u64::from(width) * u64::from(height) * RGBA_BYTES_PER_PIXEL;
    if width > MAX_IMPORT_SIDE || height > MAX_IMPORT_SIDE || allocation > MAX_IMPORT_ALLOC {
        return Err(ImportError::TooLarge { width, height });
    }
    Ok(())
}

fn import_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMPORT_SIDE);
    limits.max_image_height = Some(MAX_IMPORT_SIDE);
    limits.max_alloc = Some(MAX_IMPORT_ALLOC);
    limits
}
