use aurora_asset::{RgbaImage, TextureFormat};

/// What the editor shows for one entry of a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SlotPreview {
    #[default]
    Empty,
    Pixels(RgbaImage),
    /// The entry is kept byte for byte but cannot be shown.
    Unsupported(TextureFormat),
}
