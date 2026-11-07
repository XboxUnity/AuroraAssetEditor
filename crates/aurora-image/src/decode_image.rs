use std::io::Cursor;

use aurora_asset::RgbaImage;
use aurora_asset::dds;
use image::{DynamicImage, ImageBuffer, ImageError, ImageFormat, ImageReader, Limits, Luma, Rgba};

use crate::gdiplus_gray16_table::GDIPLUS_GRAY16_THRESHOLDS;
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
const BYTE_BITS: u16 = 8;

/// Decodes any import format to straight-alpha RGBA8, reducing 16-bit PNG samples to 8 bits as
/// GDI+ does. `file_name` only matters for TGA, which has no magic number.
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
        Ok(image) => Ok(to_rgba8(image, format)),
        Err(ImageError::Limits(_)) => Err(ImportError::TooLarge { width, height }),
        Err(error) => Err(ImportError::Decode(error)),
    }
}

/// 16-bit PNG samples are reduced the way GDI+ reduces them; everything else converts as `image`
/// does, which matches GDI+ for 8-bit data.
fn to_rgba8(image: DynamicImage, format: ImageFormat) -> RgbaImage {
    if format != ImageFormat::Png {
        return image.into_rgba8();
    }
    match image {
        DynamicImage::ImageLuma16(grey) => gdiplus_grey(&grey),
        DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_) => high_bytes(&image),
        _ => image.into_rgba8(),
    }
}

/// GDI+ keeps the high byte of each 16-bit sample of colour and grey-with-alpha PNGs, where
/// `image` rounds to the nearest value.
fn high_bytes(image: &DynamicImage) -> RgbaImage {
    let wide = image.to_rgba16();
    RgbaImage::from_fn(wide.width(), wide.height(), |x, y| {
        Rgba(
            wide.get_pixel(x, y)
                .0
                .map(|sample| (sample >> BYTE_BITS) as u8),
        )
    })
}

/// GDI+ maps 16-bit grey without alpha through its own curve, neither high byte nor rounding;
/// the measured table holds where each 8-bit value starts.
fn gdiplus_grey(grey: &ImageBuffer<Luma<u16>, Vec<u16>>) -> RgbaImage {
    RgbaImage::from_fn(grey.width(), grey.height(), |x, y| {
        let sample = grey.get_pixel(x, y)[0];
        let value = GDIPLUS_GRAY16_THRESHOLDS.partition_point(|&start| start <= sample) as u8;
        Rgba([value, value, value, u8::MAX])
    })
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
