use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use aurora_asset::{
    AssetFile, AssetSlot, Compression, RgbaImage, ScreenshotNumber, TextureFormat, encode,
};
use aurora_asset_editor::app::{App, Page};
use aurora_asset_editor::dialogs::{ChoiceId, Modal, ModalAnswer, PendingChoice};
use aurora_asset_editor::editor::SlotPreview;
use aurora_asset_editor::files::image_open_filters;
use aurora_asset_editor::platform::OpenRequest;
use aurora_asset_editor::status::{Severity, StatusLine};
use editor_test_support::{AppHarness, DialogRequest};
use image::{ImageFormat, Rgba};
use tempfile::TempDir;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

fn png_bytes(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    solid(width, height, color)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn png(folder: &TempDir, name: &str, width: u32, height: u32, color: [u8; 4]) -> PathBuf {
    let path = folder.path().join(name);
    fs::write(&path, png_bytes(width, height, color)).unwrap();
    path
}

fn number(position: u8) -> ScreenshotNumber {
    ScreenshotNumber::new(position).unwrap()
}

fn shot(position: u8) -> AssetSlot {
    AssetSlot::Screenshot(number(position))
}

fn harness_with_resize(auto_resize: bool) -> AppHarness {
    let mut harness = AppHarness::new();
    harness.app().set_auto_resize(auto_resize);
    harness
}

fn document_of(app: &App, slot: AssetSlot) -> &aurora_asset_editor::editor::AssetDocument {
    match slot {
        AssetSlot::Cover => &app.pages().cover,
        AssetSlot::Background => &app.pages().background,
        AssetSlot::Icon | AssetSlot::Banner => &app.pages().icon_banner,
        _ => &app.pages().screenshots,
    }
}

fn pixels(app: &App, slot: AssetSlot) -> RgbaImage {
    match document_of(app, slot).preview(slot) {
        SlotPreview::Pixels(pixels) => pixels.clone(),
        other => panic!("expected pixels in {slot:?}, got {other:?}"),
    }
}

fn is_empty(app: &App, slot: AssetSlot) -> bool {
    *document_of(app, slot).preview(slot) == SlotPreview::Empty
}

/// Width, height and format of the stored texture.
fn texture(app: &App, slot: AssetSlot) -> (u32, u32, TextureFormat) {
    let header = document_of(app, slot)
        .texture_header(slot)
        .unwrap_or_else(|| panic!("{slot:?} holds no texture"));
    (header.width(), header.height(), header.format())
}

fn replace(harness: &mut AppHarness, slot: AssetSlot, path: &Path) {
    harness.dialogs().push_files(vec![path.to_path_buf()]);
    harness.app().replace_image(slot);
    harness.run_jobs();
}

fn add(harness: &mut AppHarness, paths: &[PathBuf]) {
    harness.dialogs().push_files(paths.to_vec());
    harness.app().add_screenshots();
    harness.run_jobs();
}

fn drop_files(harness: &mut AppHarness, paths: &[PathBuf]) {
    harness.app().files_dropped(paths.to_vec());
}

fn message_title(app: &App) -> String {
    match app.modal() {
        Some(Modal::Message { title, .. }) => title.clone(),
        other => panic!("expected a message dialog, got {other:?}"),
    }
}

fn message_text(app: &App) -> String {
    match app.modal() {
        Some(Modal::Message { text, .. }) => text.clone(),
        other => panic!("expected a message dialog, got {other:?}"),
    }
}

#[test]
fn replace_stretches_to_target_when_auto_resize_on() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    let path = png(&folder, "wide.png", 100, 50, RED);

    replace(&mut harness, AssetSlot::Cover, &path);

    let app = &mut *harness.app();
    assert_eq!(pixels(app, AssetSlot::Cover).dimensions(), (900, 600));
    let (width, height, _) = texture(app, AssetSlot::Cover);
    assert_eq!((width, height), (900, 600));
}

#[test]
fn replace_keeps_size_when_auto_resize_off() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let path = png(&folder, "wide.png", 100, 50, RED);

    replace(&mut harness, AssetSlot::Cover, &path);

    let app = &mut *harness.app();
    assert_eq!(pixels(app, AssetSlot::Cover), solid(100, 50, RED));
    let (width, height, _) = texture(app, AssetSlot::Cover);
    assert_eq!((width, height), (100, 50));
}

#[test]
fn image_at_target_size_passes_through_untouched() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    let path = png(&folder, "exact.png", 900, 600, BLUE);

    replace(&mut harness, AssetSlot::Cover, &path);

    assert_eq!(
        pixels(&harness.app(), AssetSlot::Cover),
        solid(900, 600, BLUE)
    );
}

#[test]
fn compression_applies_when_image_is_loaded() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let first = png(&folder, "first.png", 16, 16, RED);
    let second = png(&folder, "second.png", 16, 16, GREEN);

    harness.app().set_use_compression(true);
    replace(&mut harness, AssetSlot::Icon, &first);
    assert_eq!(
        texture(&harness.app(), AssetSlot::Icon).2,
        TextureFormat::LinearDxt4
    );

    harness.app().set_use_compression(false);
    assert_eq!(
        texture(&harness.app(), AssetSlot::Icon).2,
        TextureFormat::LinearDxt4
    );

    replace(&mut harness, AssetSlot::Banner, &second);
    assert_eq!(
        texture(&harness.app(), AssetSlot::Banner).2,
        TextureFormat::LinearArgb8888
    );
    assert_eq!(
        texture(&harness.app(), AssetSlot::Icon).2,
        TextureFormat::LinearDxt4
    );
}

#[test]
fn failed_image_load_keeps_existing_image() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let good = png(&folder, "good.png", 20, 10, RED);
    replace(&mut harness, AssetSlot::Cover, &good);
    let before_texture = texture(&harness.app(), AssetSlot::Cover);
    let before_revision = harness.app().pages().cover.revision(AssetSlot::Cover);

    let broken = folder.path().join("broken.png");
    fs::write(&broken, b"this is not an image").unwrap();
    replace(&mut harness, AssetSlot::Cover, &broken);

    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    assert_eq!(message_title(app), "Could not use image");
    assert!(
        message_text(app).starts_with("There was an error while trying to process broken.png: "),
        "{}",
        message_text(app)
    );
    assert_eq!(pixels(app, AssetSlot::Cover), solid(20, 10, RED));
    assert_eq!(texture(app, AssetSlot::Cover), before_texture);
    assert_eq!(
        app.pages().cover.revision(AssetSlot::Cover),
        before_revision
    );
    drop(app_guard);
    assert!(harness.error_log_text().contains("broken.png"));
}

#[test]
fn oversized_image_with_auto_resize_off_is_rejected() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let good = png(&folder, "good.png", 20, 10, RED);
    replace(&mut harness, AssetSlot::Background, &good);
    let wide = png(&folder, "wide.png", 9000, 10, GREEN);

    replace(&mut harness, AssetSlot::Background, &wide);

    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    assert_eq!(message_title(app), "Could not use image");
    assert!(
        message_text(app).contains("image size 9000x10 is outside the supported range"),
        "{}",
        message_text(app)
    );
    assert_eq!(pixels(app, AssetSlot::Background), solid(20, 10, RED));
    drop(app_guard);
    assert!(harness.error_log_text().contains("wide.png"));
}

#[test]
fn dropping_a_folder_is_reported() {
    let folder = TempDir::new().unwrap();
    let subfolder = folder.path().join("covers");
    fs::create_dir(&subfolder).unwrap();
    let mut harness = harness_with_resize(true);

    drop_files(&mut harness, &[subfolder]);
    assert_eq!(harness.queued_jobs(), 1);
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(message_title(app), "Could not load files");
    assert_eq!(
        message_text(app),
        "covers: this is a folder, not a file\n\nDetails are in error.log."
    );
    assert!(is_empty(app, AssetSlot::Cover));
}

#[test]
fn image_dialogs_list_new_formats() {
    let mut harness = harness_with_resize(true);
    let replace_cases = [
        (AssetSlot::Cover, "Select new cover", "cover.png"),
        (
            AssetSlot::Background,
            "Select new background",
            "background.png",
        ),
        (AssetSlot::Icon, "Select new icon", "icon.png"),
        (AssetSlot::Banner, "Select new banner", "banner.png"),
        (shot(1), "Select new screenshot", "screenshot.png"),
    ];
    for (slot, _, _) in &replace_cases {
        harness.dialogs().push_cancel();
        harness.app().replace_image(*slot);
    }
    harness.dialogs().push_cancel();
    harness.app().add_screenshots();

    let filters = image_open_filters();
    let mut expected: Vec<DialogRequest> = replace_cases
        .iter()
        .map(|(_, title, name)| DialogRequest::OpenFile(open_request(title, name, &filters)))
        .collect();
    expected.push(DialogRequest::OpenFiles(open_request(
        "Select new screenshot(s)",
        "screenshot.png",
        &filters,
    )));
    assert_eq!(harness.dialogs().requests(), expected);

    let names: Vec<&str> = filters.iter().map(|filter| filter.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "All supported images",
            "PNG",
            "JPEG",
            "BMP",
            "GIF",
            "TIFF",
            "WebP",
            "TGA",
            "ICO",
            "QOI",
            "DDS",
            "All files"
        ]
    );
    assert_eq!(
        filters[0].patterns,
        [
            "*.png", "*.jpg", "*.jpeg", "*.bmp", "*.gif", "*.tif", "*.tiff", "*.webp", "*.tga",
            "*.ico", "*.qoi", "*.dds"
        ]
    );
    assert_eq!(filters[2].patterns, ["*.jpg", "*.jpeg"]);
    assert_eq!(filters[5].patterns, ["*.tif", "*.tiff"]);
    assert_eq!(filters[11].patterns, ["*"]);
}

fn open_request(
    title: &str,
    default_name: &str,
    filters: &[aurora_asset_editor::platform::FileFilter],
) -> OpenRequest {
    OpenRequest {
        title: title.to_owned(),
        filters: filters.to_vec(),
        default_filter: 0,
        default_name: Some(default_name.to_owned()),
    }
}

#[test]
fn add_fills_free_slots_in_order() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let first = png(&folder, "first.png", 8, 8, RED);
    harness.app().select_screenshot(number(1));
    replace(&mut harness, shot(1), &first);
    harness.app().select_screenshot(number(3));
    replace(&mut harness, shot(3), &first);
    let new_files = [
        png(&folder, "a.png", 9, 9, RED),
        png(&folder, "b.png", 10, 10, GREEN),
        png(&folder, "c.png", 11, 11, BLUE),
    ];

    add(&mut harness, &new_files);

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(2)), solid(9, 9, RED));
    assert_eq!(pixels(app, shot(4)), solid(10, 10, GREEN));
    assert_eq!(pixels(app, shot(5)), solid(11, 11, BLUE));
    assert_eq!(pixels(app, shot(1)), solid(8, 8, RED));
    assert_eq!(app.pages().screenshots.screenshot_count(), 5);
    assert_eq!(app.selected_screenshot(), number(5));
}

#[test]
fn adding_more_screenshots_than_free_slots_warns_once() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let files: Vec<PathBuf> = (0..20)
        .map(|index| png(&folder, &format!("s{index}.png"), 8, 8, RED))
        .collect();
    add(&mut harness, &files);
    assert_eq!(harness.app().pages().screenshots.screenshot_count(), 20);
    assert!(harness.app().modal().is_none());
    harness.app().remove_image(shot(7));

    let extra = [
        png(&folder, "x.png", 9, 9, GREEN),
        png(&folder, "y.png", 10, 10, BLUE),
        png(&folder, "z.png", 11, 11, BLUE),
    ];
    add(&mut harness, &extra);

    let app = &mut *harness.app();
    assert_eq!(message_title(app), "No space left");
    assert_eq!(
        message_text(app),
        "There is no space left for new screenshots :("
    );
    assert_eq!(pixels(app, shot(7)), solid(9, 9, GREEN));
    app.answer_modal(ModalAnswer::Accept);
    assert!(app.modal().is_none());
}

#[test]
fn replace_screenshot_targets_selected_slot() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let path = png(&folder, "new.png", 8, 8, RED);
    assert_eq!(harness.app().selected_screenshot(), number(1));

    replace(&mut harness, shot(4), &path);

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(4)), solid(8, 8, RED));
    assert_eq!(app.pages().screenshots.screenshot_count(), 1);
    assert_eq!(app.selected_screenshot(), number(4));
}

#[test]
fn replace_on_an_empty_selected_slot_works() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let first = png(&folder, "first.png", 8, 8, RED);
    let second = png(&folder, "second.png", 9, 9, GREEN);
    replace(&mut harness, shot(1), &first);
    harness.app().select_screenshot(number(2));
    assert!(is_empty(&harness.app(), shot(2)));

    replace(&mut harness, shot(2), &second);

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(2)), solid(9, 9, GREEN));
    assert_eq!(pixels(app, shot(1)), solid(8, 8, RED));
    assert_eq!(app.selected_screenshot(), number(2));
}

#[test]
fn removing_an_empty_slot_changes_nothing() {
    let mut harness = harness_with_resize(false);
    let revision = harness.app().pages().cover.revision(AssetSlot::Cover);

    harness.app().remove_image(AssetSlot::Cover);

    assert_eq!(
        harness.app().pages().cover.revision(AssetSlot::Cover),
        revision
    );
}

#[test]
fn only_the_last_image_dropped_on_cover_is_imported() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    harness.app().show_page(Page::Cover);
    let good = png(&folder, "good.png", 8, 8, RED);
    replace(&mut harness, AssetSlot::Cover, &good);
    let first = png(&folder, "first.png", 9, 9, GREEN);
    let broken = folder.path().join("broken.png");
    fs::write(&broken, b"this is not an image").unwrap();

    drop_files(&mut harness, &[first, broken]);
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(message_title(app), "Could not use image");
    assert_eq!(pixels(app, AssetSlot::Cover), solid(8, 8, RED));
}

#[test]
fn banner_answer_imports_the_file_as_banner() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    harness.app().show_page(Page::IconBanner);
    let path = png(&folder, "wide.png", 420, 96, GREEN);

    drop_files(&mut harness, &[path]);
    harness
        .app()
        .answer_modal(ModalAnswer::Choice(ChoiceId::Banner));
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(pixels(app, AssetSlot::Banner), solid(420, 96, GREEN));
    assert!(is_empty(app, AssetSlot::Icon));
}

#[test]
fn accept_on_a_choice_cancels_it() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    harness.app().show_page(Page::IconBanner);
    let path = png(&folder, "icon.png", 64, 64, GREEN);

    drop_files(&mut harness, &[path]);
    harness.app().answer_modal(ModalAnswer::Accept);

    assert!(harness.app().modal().is_none());
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn answer_the_dialog_does_not_offer_keeps_it_open_and_is_reported() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    harness.app().show_page(Page::IconBanner);
    let first = png(&folder, "first.png", 64, 64, GREEN);
    let second = png(&folder, "second.png", 64, 64, RED);
    drop_files(&mut harness, &[first, second]);
    let before = harness.app().modal().cloned();

    harness
        .app()
        .answer_modal(ModalAnswer::Choice(ChoiceId::Replace));

    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    assert_eq!(app.modal().cloned(), before);
    assert_eq!(app.status().severity, Severity::Error);
    assert!(
        app.status()
            .text
            .contains("the dialog has no Replace button"),
        "{}",
        app.status().text
    );
    drop(app_guard);
    assert!(harness.error_log_text().contains("no Replace button"));
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn remove_image_empties_slot_without_confirmation() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let path = png(&folder, "cover.png", 8, 8, RED);
    replace(&mut harness, AssetSlot::Cover, &path);
    replace(&mut harness, AssetSlot::Icon, &path);
    replace(&mut harness, AssetSlot::Banner, &path);

    harness.app().remove_image(AssetSlot::Cover);
    harness.app().remove_image(AssetSlot::Icon);

    let app = &mut *harness.app();
    assert!(app.modal().is_none());
    assert!(is_empty(app, AssetSlot::Cover));
    assert!(app.pages().cover.texture_header(AssetSlot::Cover).is_none());
    assert!(is_empty(app, AssetSlot::Icon));
    assert!(!is_empty(app, AssetSlot::Banner));
}

#[test]
fn moving_a_screenshot_moves_its_bytes() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let files = [
        png(&folder, "one.png", 8, 8, RED),
        png(&folder, "two.png", 10, 10, GREEN),
        png(&folder, "three.png", 12, 12, BLUE),
    ];
    add(&mut harness, &files);

    harness.app().move_screenshot(number(1), number(3));

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(1)), solid(10, 10, GREEN));
    assert_eq!(pixels(app, shot(2)), solid(12, 12, BLUE));
    assert_eq!(pixels(app, shot(3)), solid(8, 8, RED));
    assert_eq!(texture(app, shot(1)).0, 10);
    assert_eq!(texture(app, shot(2)).0, 12);
    assert_eq!(texture(app, shot(3)).0, 8);
    assert_eq!(app.selected_screenshot(), number(3));
}

#[test]
fn image_import_runs_as_a_job() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let path = png(&folder, "cover.png", 8, 8, RED);
    harness.dialogs().push_files(vec![path]);

    harness.app().replace_image(AssetSlot::Cover);

    assert_eq!(harness.queued_jobs(), 1);
    assert!(is_empty(&harness.app(), AssetSlot::Cover));
    assert!(harness.app().locks().editor);

    harness.run_jobs();

    assert_eq!(pixels(&harness.app(), AssetSlot::Cover), solid(8, 8, RED));
    assert!(!harness.app().locks().editor);
}

#[test]
fn dropping_image_on_cover_replaces_it() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    harness.app().show_page(Page::Cover);
    let path = png(&folder, "dropped.png", 100, 50, GREEN);

    drop_files(&mut harness, &[path]);
    assert!(harness.app().modal().is_none());
    harness.run_jobs();

    assert_eq!(
        pixels(&harness.app(), AssetSlot::Cover).dimensions(),
        (900, 600)
    );
}

#[test]
fn dropping_on_icon_banner_asks_per_file() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    harness.app().show_page(Page::IconBanner);
    let first = png(&folder, "first.png", 64, 64, RED);
    let second = png(&folder, "second.png", 420, 96, GREEN);

    drop_files(&mut harness, &[first.clone(), second.clone()]);

    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    assert_eq!(
        app.modal(),
        Some(&Modal::Choice {
            title: "Icon or banner?".to_owned(),
            text: "Use first.png as the icon or as the banner?".to_owned(),
            choices: vec![
                (ChoiceId::Icon, "Icon".to_owned()),
                (ChoiceId::Banner, "Banner".to_owned())
            ],
            pending: PendingChoice::IconOrBanner {
                path: first,
                remaining: vec![second.clone()],
            },
        })
    );
    app.answer_modal(ModalAnswer::Choice(ChoiceId::Icon));
    assert!(matches!(
        app.modal(),
        Some(Modal::Choice { text, .. }) if text == "Use second.png as the icon or as the banner?"
    ));
    app.answer_modal(ModalAnswer::Cancel);
    assert!(app.modal().is_none());
    drop(app_guard);
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(pixels(app, AssetSlot::Icon), solid(64, 64, RED));
    assert!(is_empty(app, AssetSlot::Banner));
}

#[test]
fn dropping_screenshots_asks_once_and_fills_free_slots() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    harness.app().show_page(Page::Screenshots);
    let existing = png(&folder, "existing.png", 8, 8, RED);
    replace(&mut harness, shot(1), &existing);
    let dropped = [
        png(&folder, "a.png", 9, 9, GREEN),
        png(&folder, "b.png", 10, 10, BLUE),
        png(&folder, "c.png", 11, 11, BLUE),
    ];

    drop_files(&mut harness, &dropped);

    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    match app.modal() {
        Some(Modal::Choice {
            title,
            text,
            choices,
            ..
        }) => {
            assert_eq!(title, "Replace screenshot?");
            assert_eq!(
                text,
                "Do you want to replace the current Screenshot with a.png?"
            );
            assert_eq!(
                choices,
                &vec![
                    (ChoiceId::Replace, "Replace".to_owned()),
                    (ChoiceId::AddToFreeSlots, "Add to free slots".to_owned())
                ]
            );
        }
        other => panic!("expected the replace question, got {other:?}"),
    }
    app.answer_modal(ModalAnswer::Choice(ChoiceId::Replace));
    assert!(app.modal().is_none());
    drop(app_guard);
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(1)), solid(9, 9, GREEN));
    assert_eq!(pixels(app, shot(2)), solid(10, 10, BLUE));
    assert_eq!(pixels(app, shot(3)), solid(11, 11, BLUE));
}

#[test]
fn answering_add_puts_all_dropped_screenshots_in_free_slots() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    harness.app().show_page(Page::Screenshots);
    let existing = png(&folder, "existing.png", 8, 8, RED);
    replace(&mut harness, shot(1), &existing);
    let revision = harness.app().pages().screenshots.revision(shot(1));
    let dropped = [
        png(&folder, "a.png", 9, 9, GREEN),
        png(&folder, "b.png", 10, 10, BLUE),
    ];

    drop_files(&mut harness, &dropped);
    harness
        .app()
        .answer_modal(ModalAnswer::Choice(ChoiceId::AddToFreeSlots));
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(1)), solid(8, 8, RED));
    assert_eq!(app.pages().screenshots.revision(shot(1)), revision);
    assert_eq!(pixels(app, shot(2)), solid(9, 9, GREEN));
    assert_eq!(pixels(app, shot(3)), solid(10, 10, BLUE));
}

#[test]
fn cancel_on_screenshot_question_loads_nothing() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    harness.app().show_page(Page::Screenshots);
    let existing = png(&folder, "existing.png", 8, 8, RED);
    replace(&mut harness, shot(1), &existing);
    let dropped = [
        png(&folder, "a.png", 9, 9, GREEN),
        png(&folder, "b.png", 10, 10, BLUE),
    ];

    drop_files(&mut harness, &dropped);
    harness.app().answer_modal(ModalAnswer::Cancel);

    assert_eq!(harness.queued_jobs(), 0);
    assert!(harness.app().modal().is_none());
    assert_eq!(harness.app().pages().screenshots.screenshot_count(), 1);
}

#[test]
fn dropping_on_empty_selected_slot_does_not_ask() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    harness.app().show_page(Page::Screenshots);
    harness.app().select_screenshot(number(3));
    let dropped = [
        png(&folder, "a.png", 9, 9, GREEN),
        png(&folder, "b.png", 10, 10, BLUE),
    ];

    drop_files(&mut harness, &dropped);
    assert!(harness.app().modal().is_none());
    harness.run_jobs();

    let app = &mut *harness.app();
    assert_eq!(pixels(app, shot(3)), solid(9, 9, GREEN));
    assert_eq!(pixels(app, shot(1)), solid(10, 10, BLUE));
    assert_eq!(app.pages().screenshots.screenshot_count(), 2);
}

#[test]
fn images_dropped_on_online_or_console_are_ignored_with_note() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    let path = png(&folder, "dropped.png", 8, 8, RED);
    for page in [Page::Online, Page::Console] {
        harness.app().show_page(page);

        drop_files(&mut harness, std::slice::from_ref(&path));

        let mut app_guard = harness.app();
        let app = &mut *app_guard;
        assert_eq!(
            *app.status(),
            StatusLine::info(
                "Images can only be dropped on Cover, Background, Icon & Banner or Screenshots."
            )
        );
        assert_eq!(app.status().severity, Severity::Info);
        assert!(app.modal().is_none());
        drop(app_guard);
        assert_eq!(harness.queued_jobs(), 0);
    }
}

/// A stretched solid image has the target size and its middle is still the colour.
fn assert_green_stretched(image: &RgbaImage, size: (u32, u32)) {
    assert_eq!(image.dimensions(), size);
    assert_eq!(image.get_pixel(size.0 / 2, size.1 / 2).0, GREEN);
}

#[test]
fn asset_files_dropped_on_any_page_load() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(true);
    let mut file = AssetFile::new();
    let texture = encode(&solid(8, 8, RED), Compression::None).unwrap();
    file.set(AssetSlot::Cover, Some(texture));
    let asset = folder.path().join("GC4D5307E6.asset");
    fs::write(&asset, file.to_bytes()).unwrap();
    let image = png(&folder, "ignored.png", 8, 8, GREEN);
    for page in [
        Page::Online,
        Page::Console,
        Page::Screenshots,
        Page::Background,
    ] {
        harness.app().show_page(page);
        harness.app().remove_image(AssetSlot::Cover);

        drop_files(&mut harness, &[asset.clone(), image.clone()]);
        harness.run_jobs();

        let app = &mut *harness.app();
        assert!(!is_empty(app, AssetSlot::Cover), "{page:?}");
        assert_eq!(app.page(), Page::Cover);
        match page {
            Page::Background => {
                assert_green_stretched(&pixels(app, AssetSlot::Background), (1280, 720))
            }
            Page::Screenshots => assert_green_stretched(&pixels(app, shot(1)), (1000, 562)),
            _ => {}
        }
        if matches!(page, Page::Online | Page::Console) {
            assert!(
                app.status().text.starts_with("Loaded GC4D5307E6.asset"),
                "{}",
                app.status().text
            );
        }
    }
}

#[test]
fn drop_during_import_is_refused() {
    let folder = TempDir::new().unwrap();
    let mut harness = harness_with_resize(false);
    let path = png(&folder, "cover.png", 8, 8, RED);
    harness.dialogs().push_files(vec![path.to_path_buf()]);
    harness.app().replace_image(AssetSlot::Cover);
    assert!(harness.app().locks().editor);

    drop_files(&mut harness, std::slice::from_ref(&path));
    harness.app().open_paths(vec![path]);

    assert_eq!(harness.queued_jobs(), 1);
    assert!(harness.app().modal().is_none());
    assert_eq!(
        *harness.app().status(),
        StatusLine::info("Busy, try again when the current work finishes")
    );
    harness.run_jobs();
    assert_eq!(pixels(&harness.app(), AssetSlot::Cover), solid(8, 8, RED));
}

#[test]
fn drop_hover_ends_with_the_drop() {
    let mut harness = harness_with_resize(true);
    assert!(!harness.app().drop_hover());
    harness.app().set_drop_hover(true);
    assert!(harness.app().drop_hover());

    harness.app().files_dropped(Vec::new());

    assert!(!harness.app().drop_hover());
}
