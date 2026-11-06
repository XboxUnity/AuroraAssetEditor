#![forbid(unsafe_code)]

mod texture;

pub use texture::{
    Compression, EncodeError, MAX_DIMENSION, TEXTURE_HEADER_LEN, TextureFormat, TextureHeader,
};
