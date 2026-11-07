use aurora_asset::{TextureFormat, TextureHeader};

const UNSUPPORTED_CAPTION: &str = "Unsupported texture format (kept as is)";

/// The page header's "{w} x {h} · {format}" text for a stored texture.
pub fn texture_caption(header: &TextureHeader) -> String {
    let format = match header.format() {
        TextureFormat::LinearDxt4 => "DXT",
        TextureFormat::LinearDxt1 => "DXT1",
        TextureFormat::LinearDxt5 => "DXT5",
        TextureFormat::LinearArgb8888 => "ARGB",
        TextureFormat::Unsupported { .. } => UNSUPPORTED_CAPTION,
    };
    format!("{} x {} · {format}", header.width(), header.height())
}
