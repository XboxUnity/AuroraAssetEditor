use aurora_asset::{Compression, EncodeError, EncodedTexture, RgbaImage, encode};
use aurora_image::{ImageTarget, encoder_pixels, fit_to_target};

/// Pixels ready to become an asset entry: what the editor shows and the texture it stores.
/// Every path that puts an image on a page goes through here, so the encoder only ever sees
/// `encoder_pixels`.
#[derive(Debug, Clone)]
pub struct PreparedImage {
    /// The image after any resize, with exact straight alpha.
    pub pixels: RgbaImage,
    pub texture: EncodedTexture,
}

impl PreparedImage {
    pub fn prepare(
        source: RgbaImage,
        target: ImageTarget,
        auto_resize: bool,
        compression: Compression,
    ) -> Result<PreparedImage, EncodeError> {
        Self::without_resize(fit_to_target(source, target, auto_resize), compression)
    }

    pub fn without_resize(
        source: RgbaImage,
        compression: Compression,
    ) -> Result<PreparedImage, EncodeError> {
        let texture = encode(&encoder_pixels(&source), compression)?;
        Ok(PreparedImage {
            pixels: source,
            texture,
        })
    }
}
