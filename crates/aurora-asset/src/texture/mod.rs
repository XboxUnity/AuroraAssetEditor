mod argb;
mod bc3_encode;
mod block_decode;
mod channel;
mod channel_order;
mod compression;
mod decode_error;
mod encode_error;
mod encoded_texture;
mod fetch_format;
mod gpu_endian;
mod surface_layout;
mod texture_format;
mod texture_header;

use image::RgbaImage;

use surface_layout::SurfaceLayout;

pub(crate) use block_decode::{decode_bc1_block, decode_bc3_block};
pub use compression::Compression;
pub use decode_error::DecodeError;
pub use encode_error::EncodeError;
pub use encoded_texture::EncodedTexture;
pub use texture_format::TextureFormat;
pub use texture_header::{TEXTURE_HEADER_LEN, TextureHeader};

pub const MAX_DIMENSION: u32 = 8192;

/// Encodes an image byte for byte as AuroraAsset.dll does: the header it writes and its
/// zero-padded surface, BC3 (DXT4) for `Compression::Dxt` or A8R8G8B8 for `Compression::None`.
pub fn encode(image: &RgbaImage, compression: Compression) -> Result<EncodedTexture, EncodeError> {
    let header = TextureHeader::for_image(image.width(), image.height(), compression)?;
    let layout = SurfaceLayout::for_compression(image.width(), image.height(), compression);
    let data = match compression {
        Compression::None => argb::pack(image, &layout),
        Compression::Dxt => bc3_encode::encode_surface(image, &layout),
    };
    Ok(EncodedTexture { header, data })
}

/// Decodes a texture to straight-alpha RGBA pixels equal to AuroraAsset.dll's decode.
pub fn decode(texture: &EncodedTexture) -> Result<RgbaImage, DecodeError> {
    let header = &texture.header;
    let format = header.format();
    let layout = SurfaceLayout::for_header(header)?;
    if let Some(codec) = format.block_codec() {
        let channels = header
            .channel_order()
            .ok_or(DecodeError::UnsupportedFormat(format))?;
        return block_decode::decode_surface(
            &texture.data,
            header.width(),
            header.height(),
            &layout,
            codec,
            header.gpu_endian(),
            channels,
        );
    }
    match format {
        TextureFormat::LinearArgb8888 => {
            argb::unpack(&texture.data, header.width(), header.height(), &layout)
        }
        _ => Err(DecodeError::UnsupportedFormat(format)),
    }
}
