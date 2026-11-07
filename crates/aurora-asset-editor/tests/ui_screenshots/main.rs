//! Every screen and dialog of the app in both themes, compared pixel for pixel with the approved
//! references in `fixtures/ui`. `cargo xtask ui-screenshots check|update` runs this target.

mod sample_content;
mod shell_screens;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

use editor_test_support::{
    ReferenceMode, ReferenceStore, SCREEN_THEMES, Screen, ScreenArea, ScreenshotSuite, image_name,
};

/// The catalogue, one area per section of the contact sheet.
const AREAS: &[ScreenArea] = &[shell_screens::SHELL];

/// A screen name starts with what it shows, so a sorted listing of `fixtures/ui` groups related
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
const MAX_REFERENCES_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REFERENCE_BYTES: u64 = 200 * 1024;
const REFERENCE_EXTENSION: &str = "png";

#[test]
fn every_screen_matches_its_reference() {
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

/// Checks every screen whatever the filter. An update run skips it: a filtered update leaves the
/// other screens without references, and the suite writes references while this test reads them.
#[test]
fn every_screen_has_both_references() {
    let mode = ReferenceMode::from_environment().unwrap_or_else(|error| panic!("{error}"));
    if mode == ReferenceMode::Update {
        println!("every_screen_has_both_references is skipped in update mode");
        return;
    }
    let store = ReferenceStore::for_workspace();
    let missing: Vec<String> = all_images()
        .filter(|image| !store.reference_path(image).is_file())
        .collect();
    assert!(
        missing.is_empty(),
        "no reference in {} for {}",
        store.references().display(),
        missing.join(", ")
    );
}

#[test]
fn references_fit_the_size_budget() {
    let store = ReferenceStore::for_workspace();
    let sizes = reference_sizes(store.references());
    let oversized: Vec<String> = sizes
        .iter()
        .filter(|(_, size)| *size > MAX_REFERENCE_BYTES)
        .map(|(file, size)| format!("{file} ({size} bytes)"))
        .collect();
    assert!(
        oversized.is_empty(),
        "references over {MAX_REFERENCE_BYTES} bytes: {}",
        oversized.join(", ")
    );
    let total: u64 = sizes.iter().map(|(_, size)| size).sum();
    assert!(
        total <= MAX_REFERENCES_BYTES,
        "the references take {total} bytes, over the budget of {MAX_REFERENCES_BYTES}"
    );
}

fn all_screens() -> impl Iterator<Item = &'static Screen> {
    AREAS.iter().flat_map(|area| area.screens.iter())
}

fn all_images() -> impl Iterator<Item = String> {
    all_screens().flat_map(|screen| {
        SCREEN_THEMES
            .iter()
            .map(move |(_, theme_name)| image_name(screen, theme_name))
    })
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

/// The size of every reference PNG, by file name; none when the folder does not exist yet.
fn reference_sizes(folder: &Path) -> Vec<(String, u64)> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("{} cannot be listed: {error}", folder.display()),
    };
    entries
        .map(|entry| entry.expect("the references folder can be read"))
        .filter(|entry| entry.path().extension() == Some(OsStr::new(REFERENCE_EXTENSION)))
        .map(|entry| {
            let size = entry
                .metadata()
                .expect("a reference's size can be read")
                .len();
            (entry.file_name().to_string_lossy().into_owned(), size)
        })
        .collect()
}
