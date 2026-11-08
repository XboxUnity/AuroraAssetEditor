//! Renders every screen and dialog of the app in both themes and compares each image with the
//! previous run on this platform. `cargo xtask ui-screenshots` runs this target.

mod about_screens;
mod asset_page_screens;
mod console_dialog_screens;
mod console_screens;
mod editor_dialog_screens;
mod online_screens;
mod sample_content;
mod screenshot_page_screens;
mod shell_screens;
mod staged_outcomes;

use std::collections::BTreeSet;

use editor_test_support::{Screen, ScreenArea, ScreenshotSuite};

/// The catalogue, one area per section of the contact sheet.
const AREAS: &[ScreenArea] = &[
    shell_screens::SHELL,
    asset_page_screens::ASSET_PAGES,
    screenshot_page_screens::SCREENSHOT_PAGE,
    online_screens::ONLINE,
    console_screens::CONSOLE,
    editor_dialog_screens::EDITOR_DIALOGS,
    console_dialog_screens::CONSOLE_DIALOGS,
    about_screens::ABOUT,
];

/// A screen name starts with what it shows, so a sorted listing of a run's `images/` groups related
/// screens and each file name leads to its area.
const SCREEN_NAME_PREFIXES: [&str; 9] = [
    "shell",
    "cover",
    "background",
    "icon-banner",
    "screenshots",
    "online",
    "console",
    "dialog",
    "about",
];

#[test]
fn every_screen_renders() {
    let report = ScreenshotSuite::from_environment().run(AREAS);
    assert!(report.passed(), "{report}");
}

#[test]
fn screen_names_are_unique_and_kebab_case() {
    let mut seen = BTreeSet::new();
    for screen in all_screens() {
        let name = screen.name;
        assert!(seen.insert(name), "{name} names more than one screen");
        assert!(is_kebab_case(name), "{name} is not lowercase kebab-case");
        assert!(
            SCREEN_NAME_PREFIXES
                .iter()
                .any(|prefix| starts_with_prefix(name, prefix)),
            "{name} does not start with one of {}",
            SCREEN_NAME_PREFIXES.join(", ")
        );
    }
}

fn all_screens() -> impl Iterator<Item = &'static Screen> {
    AREAS.iter().flat_map(|area| area.screens.iter())
}

/// Lowercase letters and digits in words joined by single hyphens.
fn is_kebab_case(name: &str) -> bool {
    name.split('-').all(|word| {
        !word.is_empty()
            && word
                .chars()
                .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit())
    })
}

fn starts_with_prefix(name: &str, prefix: &str) -> bool {
    name == prefix || name.starts_with(&format!("{prefix}-"))
}
