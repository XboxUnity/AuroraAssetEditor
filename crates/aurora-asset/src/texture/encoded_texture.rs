use super::texture_header::TextureHeader;

/// A texture as stored in an `.asset` entry: the 52-byte header plus the surface bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedTexture {
    pub header: TextureHeader,
    pub data: Vec<u8>,
}
