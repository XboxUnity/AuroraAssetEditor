use std::path::PathBuf;

use aurora_asset::{AssetFile, AssetSlot, Compression, RgbaImage, encode};
use aurora_image::{ImageTarget, decode_image, encoder_pixels};

fn placeholder_path(target: ImageTarget) -> PathBuf {
    let name = match target {
        ImageTarget::Cover => "ph_cover",
        ImageTarget::Background => "ph_background",
        ImageTarget::Icon => "ph_icon",
        ImageTarget::Banner => "ph_banner",
        ImageTarget::Screenshot => "ph_screenshot",
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/codec/inputs")
        .join(format!("{name}.png"))
}

/// The old app's placeholder art for `target`, at the target's size.
pub fn placeholder(target: ImageTarget) -> RgbaImage {
    let path = placeholder_path(target);
    let bytes = std::fs::read(&path).expect("placeholder fixture is readable");
    decode_image(&bytes, path.file_name().and_then(|name| name.to_str()))
        .expect("placeholder fixture decodes")
}

/// An asset file with each image encoded the way the app encodes it (through `encoder_pixels`).
pub fn asset_file_with(entries: &[(AssetSlot, RgbaImage, Compression)]) -> AssetFile {
    let mut file = AssetFile::new();
    for (slot, image, compression) in entries {
        let texture = encode(&encoder_pixels(image), *compression)
            .expect("sample image is within the encodable size range");
        file.set(*slot, Some(texture));
    }
    file
}
