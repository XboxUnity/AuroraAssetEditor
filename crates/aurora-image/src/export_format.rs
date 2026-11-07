use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, RgbaImage};

use crate::export_error::ExportError;

/// The default quality of the GDI+ JPEG encoder the old app used.
const JPEG_QUALITY: u8 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    Bmp,
    Tiff,
    Gif,
    WebP,
}

impl ExportFormat {
    /// Chosen by extension; anything else is PNG, as the old `SaveToFile` did.
    pub fn from_path(path: &Path) -> ExportFormat {
        let extension = path
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
        match extension.as_deref() {
            Some("jpg" | "jpeg") => ExportFormat::Jpeg,
            Some("bmp") => ExportFormat::Bmp,
            Some("tif" | "tiff") => ExportFormat::Tiff,
            Some("gif") => ExportFormat::Gif,
            Some("webp") => ExportFormat::WebP,
            _ => ExportFormat::Png,
        }
    }

    pub fn encode(self, image: &RgbaImage) -> Result<Vec<u8>, ExportError> {
        match self {
            ExportFormat::Png => encode_with(image, ImageFormat::Png),
            ExportFormat::Bmp => encode_with(image, ImageFormat::Bmp),
            ExportFormat::Tiff => encode_with(image, ImageFormat::Tiff),
            ExportFormat::Gif => encode_with(image, ImageFormat::Gif),
            ExportFormat::Jpeg => encode_jpeg(image),
            ExportFormat::WebP => encode_lossless_webp(image),
        }
    }
}

fn encode_with(image: &RgbaImage, format: ImageFormat) -> Result<Vec<u8>, ExportError> {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format)?;
    Ok(bytes.into_inner())
}

fn encode_jpeg(image: &RgbaImage) -> Result<Vec<u8>, ExportError> {
    let rgb = DynamicImage::ImageRgba8(image.clone()).into_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY).encode_image(&rgb)?;
    Ok(bytes)
}

fn encode_lossless_webp(image: &RgbaImage) -> Result<Vec<u8>, ExportError> {
    let mut bytes = Vec::new();
    WebPEncoder::new_lossless(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}
