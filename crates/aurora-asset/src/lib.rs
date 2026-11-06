#![forbid(unsafe_code)]

mod container;
mod texture;

pub use container::{AssetError, AssetFile, AssetKind, AssetSlot, ScreenshotNumber};
pub use image::RgbaImage;
pub use texture::{
    Compression, DecodeError, EncodeError, EncodedTexture, MAX_DIMENSION, TEXTURE_HEADER_LEN,
    TextureFormat, TextureHeader, decode, encode,
};
