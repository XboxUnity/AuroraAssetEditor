use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use aurora_asset_editor::app::Menu;
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{
    AppHarness, ExportReport, Scene, Screen, ScreenExport, read_png, write_opaque_png,
};
use image::{ImageFormat, RgbaImage};
use tempfile::TempDir;

const ALL_IMAGES: [&str; 4] = ["a-dark", "a-light", "b-dark", "b-light"];
const STALE_IMAGE_SIZE: (u32, u32) = (2, 2);
const NOTES_FILE: &str = "notes.txt";
const NOTES_TEXT: &[u8] = b"Not an image of this export.";
const OLD_IMAGE_FILE: &str = "old.png";

fn menu_scene(theme: ThemeChoice, menu: Menu) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.toggle_menu(Some(menu)));
    scene
}

fn stage_gear_menu(theme: ThemeChoice) -> Scene {
    menu_scene(theme, Menu::Gear)
}

fn stage_open_menu(theme: ThemeChoice) -> Scene {
    menu_scene(theme, Menu::Open)
}

fn stage_panicking(_theme: ThemeChoice) -> Scene {
    panic!("boom")
}

const SCREEN_A: Screen = Screen {
    name: "a",
    description: "The gear menu open",
    stage: stage_gear_menu,
};
const SCREEN_B: Screen = Screen {
    name: "b",
    description: "The open menu open",
    stage: stage_open_menu,
};
const TWO_SCREENS: &[Screen] = &[SCREEN_A, SCREEN_B];
const BROKEN_AND_B: &[Screen] = &[
    Screen {
        name: "broken",
        description: "A screen whose staging panics",
        stage: stage_panicking,
    },
    SCREEN_B,
];

/// The image a stage gives in `theme`, captured on its own thread as the export does.
fn capture(stage: fn(ThemeChoice) -> Scene, theme: ThemeChoice) -> RgbaImage {
    thread::spawn(move || stage(theme).capture())
        .join()
        .expect("the stage renders")
}

fn image_path(folder: &Path, image: &str) -> PathBuf {
    folder.join(format!("{image}.png"))
}

fn read_bytes(path: &Path) -> Vec<u8> {
    fs::read(path).expect("the file is readable")
}

fn export(folder: &Path, screens: &[Screen]) -> ExportReport {
    ScreenExport::new(folder.to_path_buf()).export(screens)
}

#[test]
fn every_screen_is_written_in_both_themes() {
    let folder = TempDir::new().expect("a temp folder");

    let report = export(folder.path(), TWO_SCREENS);

    assert!(report.passed(), "{report}");
    assert_eq!(report.written, ALL_IMAGES);
    assert!(report.kept.is_empty(), "{report}");
    let expected = [
        (
            "a-dark",
            stage_gear_menu as fn(ThemeChoice) -> Scene,
            ThemeChoice::Dark,
        ),
        ("a-light", stage_gear_menu, ThemeChoice::Light),
        ("b-dark", stage_open_menu, ThemeChoice::Dark),
        ("b-light", stage_open_menu, ThemeChoice::Light),
    ];
    for (image, stage, theme) in expected {
        let written = read_png(&image_path(folder.path(), image)).expect("the image is a PNG");
        assert!(
            written == capture(stage, theme),
            "{image} is not the capture"
        );
    }
}

#[test]
fn unchanged_image_keeps_its_bytes() {
    let folder = TempDir::new().expect("a temp folder");
    let first = export(folder.path(), TWO_SCREENS);
    assert!(first.passed(), "{first}");
    let path = image_path(folder.path(), "a-dark");
    let exported = read_bytes(&path);
    let pixels = read_png(&path).expect("the image is a PNG");
    pixels
        .save_with_format(&path, ImageFormat::Png)
        .expect("the same pixels can be written with other settings");
    let rewritten = read_bytes(&path);
    assert_ne!(rewritten, exported, "the rewrite must give other bytes");

    let second = export(folder.path(), TWO_SCREENS);

    assert!(second.passed(), "{second}");
    assert!(second.kept.contains(&"a-dark".to_owned()), "{second}");
    assert!(!second.written.contains(&"a-dark".to_owned()), "{second}");
    assert_eq!(read_bytes(&path), rewritten);
}

#[test]
fn changed_image_is_rewritten() {
    let folder = TempDir::new().expect("a temp folder");
    let path = image_path(folder.path(), "a-dark");
    let stale = RgbaImage::new(STALE_IMAGE_SIZE.0, STALE_IMAGE_SIZE.1);
    stale
        .save_with_format(&path, ImageFormat::Png)
        .expect("the stale image is written");

    let report = export(folder.path(), TWO_SCREENS);

    assert!(report.passed(), "{report}");
    assert!(report.written.contains(&"a-dark".to_owned()), "{report}");
    let written = read_png(&path).expect("the image is a PNG");
    assert!(written == capture(stage_gear_menu, ThemeChoice::Dark));
}

#[test]
fn file_that_is_not_a_png_is_rewritten() {
    let folder = TempDir::new().expect("a temp folder");
    let path = image_path(folder.path(), "a-dark");
    fs::write(&path, b"not a PNG").expect("the text file is written");

    let report = export(folder.path(), TWO_SCREENS);

    assert!(report.passed(), "{report}");
    assert!(report.written.contains(&"a-dark".to_owned()), "{report}");
    let written = read_png(&path).expect("the image is a PNG");
    assert!(written == capture(stage_gear_menu, ThemeChoice::Dark));
}

#[test]
fn a_panicking_screen_is_an_error_and_the_rest_still_go() {
    let folder = TempDir::new().expect("a temp folder");

    let report = export(folder.path(), BROKEN_AND_B);

    assert!(!report.passed());
    assert_eq!(report.errors.len(), 2, "{report}");
    for (error, image) in report.errors.iter().zip(["broken-dark", "broken-light"]) {
        assert!(error.contains(image), "{error}");
        assert!(error.contains("boom"), "{error}");
    }
    assert_eq!(report.written, ["b-dark", "b-light"]);
    assert!(image_path(folder.path(), "b-dark").is_file());
    assert!(!image_path(folder.path(), "broken-dark").exists());
}

#[test]
fn an_unreadable_file_is_an_error_and_is_left_in_place() {
    let folder = TempDir::new().expect("a temp folder");
    // A folder where the image goes cannot be read as a file, nor replaced by one.
    let blocker = image_path(folder.path(), "a-dark");
    fs::create_dir(&blocker).expect("the blocking folder is created");

    let report = export(folder.path(), TWO_SCREENS);

    assert!(!report.passed());
    assert_eq!(report.errors.len(), 1, "{report}");
    assert!(report.errors[0].contains("a-dark"), "{report}");
    assert_eq!(report.written, ["a-light", "b-dark", "b-light"]);
    assert!(blocker.is_dir());
}

#[test]
fn other_files_in_the_folder_are_left_alone() {
    let folder = TempDir::new().expect("a temp folder");
    let notes = folder.path().join(NOTES_FILE);
    fs::write(&notes, NOTES_TEXT).expect("the notes are written");
    let old = folder.path().join(OLD_IMAGE_FILE);
    write_opaque_png(&old, &capture(stage_open_menu, ThemeChoice::Dark))
        .expect("the old image is written");
    let old_bytes = read_bytes(&old);

    let report = export(folder.path(), TWO_SCREENS);

    assert!(report.passed(), "{report}");
    assert_eq!(read_bytes(&notes), NOTES_TEXT);
    assert_eq!(read_bytes(&old), old_bytes);
}

#[test]
fn report_lists_each_group_on_its_own_line() {
    let report = ExportReport {
        written: vec!["a-dark".to_owned(), "a-light".to_owned()],
        kept: Vec::new(),
        errors: vec!["b-dark: boom".to_owned(), "b-light: boom".to_owned()],
    };

    assert_eq!(
        report.to_string(),
        "Written: a-dark, a-light\nKept (pixels unchanged): none\nErrors:\n  b-dark: boom\n  b-light: boom\n"
    );
}

#[test]
fn report_without_errors_says_so() {
    let report = ExportReport {
        written: Vec::new(),
        kept: vec!["a-dark".to_owned()],
        errors: Vec::new(),
    };

    assert!(report.passed());
    assert_eq!(
        report.to_string(),
        "Written: none\nKept (pixels unchanged): a-dark\nErrors: none\n"
    );
}
