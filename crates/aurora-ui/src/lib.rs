// Not `forbid`: Slint's generated code carries `allow(unsafe_code)`, which `forbid` rejects.
#![deny(unsafe_code)]

slint::include_modules!();

/// The old app's placeholder art (PNG files) for the empty slots. The view resizes it to the space it is shown
/// in, like any picture.
pub mod placeholder_art {
    pub const COVER: &[u8] = include_bytes!("../ui/placeholders/cover.png");
    pub const BACKGROUND: &[u8] = include_bytes!("../ui/placeholders/background.png");
    pub const ICON: &[u8] = include_bytes!("../ui/placeholders/icon.png");
    pub const BANNER: &[u8] = include_bytes!("../ui/placeholders/banner.png");
    pub const SCREENSHOT: &[u8] = include_bytes!("../ui/placeholders/screenshot.png");
}
