use aurora_asset::RgbaImage;
use aurora_image::resize_preview;
use aurora_ui::PreviewArea;

use crate::view::PictureId;

/// Resizes a picture to fit the space an area of the window gives it.
#[derive(Debug)]
pub struct PreviewJob {
    pub picture: PictureId,
    pub area: PreviewArea,
    /// The space in physical pixels.
    pub space: (u32, u32),
    pub pixels: RgbaImage,
}

/// A picture resized for one area; its size is the size the area shows it at.
#[derive(Debug)]
pub struct ResizedPreview {
    pub picture: PictureId,
    pub area: PreviewArea,
    pub pixels: RgbaImage,
}

impl PreviewJob {
    pub fn run(self) -> ResizedPreview {
        ResizedPreview {
            picture: self.picture,
            area: self.area,
            pixels: resize_preview(&self.pixels, self.space),
        }
    }
}
