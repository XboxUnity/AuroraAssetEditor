use aurora_asset_editor::settings::ThemeChoice;

use super::scene::Scene;

/// Every screen is captured once per theme; the theme's name ends the image name.
pub const SCREEN_THEMES: [(ThemeChoice, &str); 2] =
    [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")];

/// One state of the UI, captured in both themes.
pub struct Screen {
    /// Lowercase kebab-case, starting with what the screen shows (`cover-loaded`).
    pub name: &'static str,
    /// What the screen shows, for the person reviewing it on the contact sheet.
    pub description: &'static str,
    /// Builds the state in the given theme. Called on a fresh thread for every image.
    pub stage: fn(ThemeChoice) -> Scene,
}

/// The screens of one part of the app: one section of the contact sheet.
pub struct ScreenArea {
    pub title: &'static str,
    pub screens: &'static [Screen],
}

/// `cover-loaded` in the theme `dark` is the image `cover-loaded-dark`.
pub fn image_name(screen: &Screen, theme_name: &str) -> String {
    format!("{}-{theme_name}", screen.name)
}
