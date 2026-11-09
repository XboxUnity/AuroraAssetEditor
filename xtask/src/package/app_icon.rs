use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use aurora_asset::RgbaImage;
use image::ImageFormat;
use image::imageops::{self, FilterType};

const APP_ICON: &str = "crates/aurora-ui/ui/app_icon.ico";
const APP_ICON_NAME: &str = "app_icon.ico";

/// The largest image of the app's icon.
pub fn load_app_icon(root: &Path) -> Result<RgbaImage> {
    let path = root.join(APP_ICON);
    let bytes = fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
    aurora_image::decode_image(&bytes, Some(APP_ICON_NAME))
        .with_context(|| format!("could not decode {}", path.display()))
}

/// The icon scaled to a square of `side` pixels.
pub fn resized_icon(icon: &RgbaImage, side: u32) -> RgbaImage {
    if icon.width() == side && icon.height() == side {
        return icon.clone();
    }
    imageops::resize(icon, side, side, FilterType::Lanczos3)
}

pub fn write_png(image: &RgbaImage, path: &Path) -> Result<()> {
    image
        .save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("could not write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notices::repository_root;

    #[test]
    fn app_icon_is_its_largest_image() {
        let icon = load_app_icon(&repository_root()).unwrap();

        assert_eq!(icon.dimensions(), (48, 48));
    }
}
