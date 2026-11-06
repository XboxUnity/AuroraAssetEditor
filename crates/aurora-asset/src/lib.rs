#![forbid(unsafe_code)]

mod texture;

pub use image::RgbaImage;
pub use texture::{
    Compression, DecodeError, EncodeError, EncodedTexture, MAX_DIMENSION, TEXTURE_HEADER_LEN,
    TextureFormat, TextureHeader, decode, dxt4_unrounded_values, encode,
};
