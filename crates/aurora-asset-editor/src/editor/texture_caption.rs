use aurora_asset::{TextureFormat, TextureHeader};

const UNSUPPORTED_CAPTION: &str = "Unsupported texture format (kept as is)";

/// The page header's "{w} x {h} · {format}" text for a stored texture.
pub fn texture_caption(header: &TextureHeader) -> String {
    format!(
        "{} x {} · {}",
        header.width(),
        header.height(),
        format_text(header)
    )
}

/// The page header's text for a stored texture whose size Aurora requires: it leads with the
/// `required` size and adds "actual {w} x {h}" when the texture is another size.
pub fn required_size_caption(header: &TextureHeader, required: (u32, u32)) -> String {
    let (width, height) = required;
    let caption = format!("{width} x {height} · {}", format_text(header));
    let actual = (header.width(), header.height());
    if actual == required {
        return caption;
    }
    format!("{caption} · actual {} x {}", actual.0, actual.1)
}

fn format_text(header: &TextureHeader) -> &'static str {
    match header.format() {
        TextureFormat::LinearDxt4 => "DXT",
        TextureFormat::LinearDxt1 => "DXT1",
        TextureFormat::LinearDxt5 => "DXT5",
        TextureFormat::LinearArgb8888 => "ARGB",
        TextureFormat::Unsupported { .. } => UNSUPPORTED_CAPTION,
    }
}
