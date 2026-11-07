#![forbid(unsafe_code)]

mod bytes;
mod container;
pub mod dds;
pub mod fsd;
mod texture;

pub use container::{
    AssetError, AssetFile, AssetKind, AssetSlot, ENTRY_COUNT, SCREENSHOT_COUNT, ScreenshotNumber,
};
pub use image::RgbaImage;
pub use texture::{
    Compression, DecodeError, EncodeError, EncodedTexture, MAX_DIMENSION, TEXTURE_HEADER_LEN,
    TextureFormat, TextureHeader, decode, encode,
};
