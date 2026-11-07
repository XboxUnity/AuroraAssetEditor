use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use aurora_asset::{
    AssetFile, AssetSlot, Compression, DecodeError, EncodedTexture, RgbaImage, TextureFormat,
    TextureHeader, decode, encode,
};
use aurora_asset_editor::app::{App, CurrentGame, DIALOG_FAILED_TITLE, Page};
use aurora_asset_editor::dialogs::{Modal, ModalAnswer};
use aurora_asset_editor::editor::{SlotPreview, texture_caption};
use aurora_asset_editor::files::{FileKind, detect};
use aurora_asset_editor::platform::{FileFilter, OpenRequest};
use aurora_asset_editor::status::{Locks, StatusLine};
use aurora_console::{GameIds, HexId};
use editor_test_support::{
    AppHarness, BACKGROUND, BANNER, COVER, DialogRequest, FsdBuilder, ICON, SCREENSHOT,
    asset_file_with,
};
use image::{ImageFormat, Rgba};
use tempfile::TempDir;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const GREY: [u8; 4] = [90, 90, 90, 255];
const TILED_FLAG_BYTE: usize = 7 * 4;
const TILED_FLAG: u8 = 0x80;

fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    solid(width, height, color)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn shot(number: u8) -> AssetSlot {
    AssetSlot::screenshot(number).unwrap()
}

fn asset_bytes(entries: &[(AssetSlot, [u8; 4])]) -> Vec<u8> {
    let images: Vec<_> = entries
        .iter()
        .map(|(slot, color)| (*slot, solid(8, 8, *color), Compression::None))
        .collect();
    asset_file_with(&images).to_bytes()
}

fn write(folder: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = folder.path().join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn load(harness: &mut AppHarness, paths: &[&Path]) {
    let paths = paths.iter().map(|path| path.to_path_buf()).collect();
    harness.app().open_paths(paths);
    harness.run_jobs();
}

fn pixels(app: &App, slot: AssetSlot) -> RgbaImage {
    let document = match slot {
        AssetSlot::Cover => &app.pages().cover,
        AssetSlot::Background => &app.pages().background,
        AssetSlot::Icon | AssetSlot::Banner => &app.pages().icon_banner,
        _ => &app.pages().screenshots,
    };
    match document.preview(slot) {
        SlotPreview::Pixels(pixels) => pixels.clone(),
        other => panic!("expected pixels in {slot:?}, got {other:?}"),
    }
}

fn is_empty(app: &App, slot: AssetSlot) -> bool {
    let document = match slot {
        AssetSlot::Cover => &app.pages().cover,
        AssetSlot::Background => &app.pages().background,
        AssetSlot::Icon | AssetSlot::Banner => &app.pages().icon_banner,
        _ => &app.pages().screenshots,
    };
    *document.preview(slot) == SlotPreview::Empty
}

fn message(title: &str, text: &str) -> Modal {
    Modal::Message {
        title: title.to_owned(),
        text: text.to_owned(),
    }
}

fn filter(name: &str, pattern: &str) -> FileFilter {
    FileFilter {
        name: name.to_owned(),
        patterns: vec![pattern.to_owned()],
    }
}

fn halo_3() -> CurrentGame {
    CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse("4D5307E6").unwrap(),
            database_id: HexId::from_database_value(18),
        },
    }
}

fn tiled_texture() -> EncodedTexture {
    let mut texture = encode(&solid(8, 8, [1, 2, 3, 255]), Compression::Dxt).unwrap();
    let mut header = texture.header.to_bytes();
    header[TILED_FLAG_BYTE] |= TILED_FLAG;
    texture.header = TextureHeader::from_bytes(header);
    texture
}

fn truncated_texture() -> EncodedTexture {
    let mut texture = encode(&solid(16, 16, [9, 9, 9, 255]), Compression::Dxt).unwrap();
    texture.data.truncate(4);
    texture
}

#[test]
fn detect_reads_the_magic_only() {
    assert_eq!(detect(b"RXEA\0\0"), FileKind::Asset);
    assert_eq!(detect(b"FSDA"), FileKind::Fsd);
    assert_eq!(detect(b"RXE"), FileKind::Unrecognised);
    assert_eq!(detect(b""), FileKind::Unrecognised);
    assert_eq!(detect(b"\x89PNG"), FileKind::Unrecognised);
}

#[test]
fn open_dialog_uses_old_title_and_prefix_filters() {
    let mut harness = AppHarness::new();
    harness.dialogs().push_cancel();

    harness.app().open_files_with_dialog();

    assert_eq!(
        harness.dialogs().requests(),
        vec![DialogRequest::OpenFiles(OpenRequest {
            title: "Select Asset(s) to load".to_owned(),
            filters: vec![
                filter("Game Cover/Boxart Asset File (GC*.asset)", "GC*.asset"),
                filter("Background Asset File (BK*.asset)", "BK*.asset"),
                filter("Icon/Banner Asset File (GL*.asset)", "GL*.asset"),
                filter("Screenshot Asset File (SS*.asset)", "SS*.asset"),
                filter("Aurora Asset Files (*.asset)", "*.asset"),
                filter("FSD Assets Files (*.assets)", "*.assets"),
                filter("All Files(*)", "*"),
            ],
            default_filter: 4,
            default_name: None,
        })]
    );
    assert_eq!(harness.queued_jobs(), 0);
    assert_eq!(harness.app().modal(), None);
    assert_eq!(harness.app().status(), &StatusLine::ready());
}

#[test]
fn picked_files_are_loaded() {
    let folder = TempDir::new().unwrap();
    let path = write(
        &folder,
        "GC4D5307E6.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let mut harness = AppHarness::new();
    harness.dialogs().push_files(vec![path]);

    harness.app().open_files_with_dialog();
    harness.run_jobs();

    assert_eq!(pixels(harness.app(), AssetSlot::Cover), solid(8, 8, RED));
}

#[test]
fn dialog_failure_is_shown_and_logged() {
    let mut harness = AppHarness::new();
    harness.dialogs().push_failure("the portal is not running");

    harness.app().open_files_with_dialog();

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            DIALOG_FAILED_TITLE,
            "The \"Select Asset(s) to load\" dialog could not be shown: the portal is not running"
        ))
    );
    assert_eq!(DIALOG_FAILED_TITLE, "Could not open the file dialog");
    assert_eq!(harness.queued_jobs(), 0);
    let log = harness.error_log_text();
    assert!(
        log.contains(
            "The \"Select Asset(s) to load\" dialog could not be shown: could not open the file dialog\n  caused by: the portal is not running\n"
        ),
        "{log}"
    );
}

#[test]
fn asset_opens_page_by_priority() {
    let folder = TempDir::new().unwrap();
    let both = asset_bytes(&[(AssetSlot::Cover, RED), (AssetSlot::Background, GREEN)]);
    let path = write(&folder, "GC4D5307E6.asset", &both);
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.page(), Page::Cover);
    assert_eq!(pixels(app, AssetSlot::Cover), solid(8, 8, RED));
    assert!(is_empty(app, AssetSlot::Background));
}

#[test]
fn asset_with_background_and_screenshots_opens_background() {
    let folder = TempDir::new().unwrap();
    let file = asset_bytes(&[(AssetSlot::Background, GREEN), (shot(1), BLUE)]);
    let path = write(&folder, "BK4D5307E6.asset", &file);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.page(), Page::Background);
    assert!(is_empty(app, shot(1)));
}

#[test]
fn asset_with_screenshots_and_icon_opens_screenshots() {
    let folder = TempDir::new().unwrap();
    let file = asset_bytes(&[(AssetSlot::Icon, GREEN), (shot(2), BLUE)]);
    let path = write(&folder, "SS4D5307E6.asset", &file);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.page(), Page::Screenshots);
    assert!(is_empty(app, AssetSlot::Icon));
}

#[test]
fn asset_with_only_icon_banner_opens_icon_banner_page() {
    let folder = TempDir::new().unwrap();
    let file = asset_bytes(&[(AssetSlot::Icon, RED), (AssetSlot::Banner, BLUE)]);
    let path = write(&folder, "GL4D5307E6.asset", &file);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.page(), Page::IconBanner);
    assert_eq!(pixels(app, AssetSlot::Icon), solid(8, 8, RED));
    assert_eq!(pixels(app, AssetSlot::Banner), solid(8, 8, BLUE));
}

#[test]
fn asset_load_copies_all_twenty_screenshots() {
    let folder = TempDir::new().unwrap();
    let colors: Vec<(AssetSlot, [u8; 4])> = (1..=20u8)
        .map(|number| (shot(number), [number * 10, 0, 0, 255]))
        .collect();
    let path = write(&folder, "SS4D5307E6.asset", &asset_bytes(&colors));
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.pages().screenshots.screenshot_count(), 20);
    for (slot, color) in colors {
        assert_eq!(pixels(app, slot), solid(8, 8, color), "{slot:?}");
    }
}

#[test]
fn asset_without_entries_reports_no_assets() {
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "GC4D5307E6.asset", &AssetFile::new().to_bytes());
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "No assets",
            "GC4D5307E6.asset doesn't contain any assets."
        ))
    );
    assert_eq!(harness.app().status(), &StatusLine::ready());
    assert!(
        harness
            .error_log_text()
            .contains("doesn't contain any assets")
    );
}

#[test]
fn undecodable_entry_is_dropped_and_reported() {
    let mut source = AssetFile::new();
    source.set(shot(2), Some(truncated_texture()));
    source.set(
        shot(3),
        Some(encode(&solid(8, 8, GREEN), Compression::None).unwrap()),
    );
    let reason = decode(&truncated_texture()).unwrap_err();
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "SS4D5307E6.asset", &source.to_bytes());
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert!(matches!(reason, DecodeError::TruncatedData { .. }));
    assert!(is_empty(app, shot(2)));
    assert_eq!(pixels(app, shot(3)), solid(8, 8, GREEN));
    assert_eq!(app.pages().screenshots.screenshot_count(), 1);
    assert_eq!(
        app.modal(),
        Some(&message(
            "Some images could not be read",
            &format!(
                "Screenshot 2 in SS4D5307E6.asset could not be decoded and was left out: {reason}"
            )
        ))
    );
    assert!(
        harness
            .error_log_text()
            .contains("Screenshot 2 in SS4D5307E6.asset could not be decoded")
    );
}

#[test]
fn unsupported_entry_is_kept_and_labelled() {
    let tiled = tiled_texture();
    let mut source = AssetFile::new();
    source.set(AssetSlot::Cover, Some(tiled.clone()));
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "GC4D5307E6.asset", &source.to_bytes());
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert!(matches!(
        app.pages().cover.preview(AssetSlot::Cover),
        SlotPreview::Unsupported(TextureFormat::Unsupported { .. })
    ));
    let header = app.pages().cover.texture_header(AssetSlot::Cover).unwrap();
    assert_eq!(
        texture_caption(header),
        "8 x 8 · Unsupported texture format (kept as is)"
    );
    assert_eq!(app.modal(), None);
}

fn full_fsd() -> Vec<u8> {
    FsdBuilder::new()
        .entry(COVER, &png(100, 50, RED))
        .entry(BACKGROUND, &png(30, 20, GREEN))
        .entry(ICON, &png(16, 16, BLUE))
        .entry(BANNER, &png(40, 10, YELLOW))
        .screenshot(&png(20, 12, RED))
        .screenshot(&png(20, 12, GREEN))
        .build()
}

#[test]
fn fsd_loads_every_type_into_free_slots() {
    let folder = TempDir::new().unwrap();
    let existing = write(&folder, "SS1.asset", &asset_bytes(&[(shot(1), GREY)]));
    let fsd = write(&folder, "game.assets", &full_fsd());
    let mut harness = AppHarness::new();
    load(&mut harness, &[&existing]);

    load(&mut harness, &[&fsd]);

    let app = harness.app();
    assert_eq!(pixels(app, AssetSlot::Cover), solid(100, 50, RED));
    assert_eq!(pixels(app, AssetSlot::Background), solid(30, 20, GREEN));
    assert_eq!(pixels(app, AssetSlot::Icon), solid(16, 16, BLUE));
    assert_eq!(pixels(app, AssetSlot::Banner), solid(40, 10, YELLOW));
    assert_eq!(pixels(app, shot(1)), solid(8, 8, GREY));
    assert_eq!(pixels(app, shot(2)), solid(20, 12, RED));
    assert_eq!(pixels(app, shot(3)), solid(20, 12, GREEN));
    assert!(is_empty(app, shot(4)));
    assert_eq!(app.modal(), None);
}

#[test]
fn fsd_without_screenshots_is_not_reported_empty() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new().entry(COVER, &png(10, 10, RED)).build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.modal(), None);
    assert_eq!(pixels(app, AssetSlot::Cover), solid(10, 10, RED));
    assert_eq!(
        app.status(),
        &StatusLine::info("Loaded game.assets · 1 of 5 asset types present")
    );
}

#[test]
fn fsd_without_images_reports_no_assets() {
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "game.assets", &FsdBuilder::new().build());
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "No assets",
            "game.assets doesn't contain any assets."
        ))
    );
}

#[test]
fn fsd_non_dds_images_decode() {
    let folder = TempDir::new().unwrap();
    let color = [10, 20, 30, 255];
    let fsd = FsdBuilder::new().entry(COVER, &png(12, 8, color)).build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(pixels(harness.app(), AssetSlot::Cover), solid(12, 8, color));
}

#[test]
fn fsd_images_are_not_resized() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new().entry(COVER, &png(100, 50, RED)).build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();
    assert!(harness.app().settings().auto_resize);

    load(&mut harness, &[&path]);

    let cover = pixels(harness.app(), AssetSlot::Cover);
    assert_eq!(cover.dimensions(), (100, 50));
    let header = harness
        .app()
        .pages()
        .cover
        .texture_header(AssetSlot::Cover)
        .unwrap();
    assert_eq!((header.width(), header.height()), (100, 50));
}

#[test]
fn fsd_images_follow_use_compression() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new().entry(COVER, &png(16, 16, RED)).build();
    let path = write(&folder, "game.assets", &fsd);
    let format_after = |use_compression: bool| {
        let mut harness = AppHarness::new();
        harness.app().set_use_compression(use_compression);
        load(&mut harness, &[&path]);
        harness
            .app()
            .pages()
            .cover
            .texture_header(AssetSlot::Cover)
            .unwrap()
            .format()
    };

    assert_eq!(format_after(true), TextureFormat::LinearDxt4);
    assert_eq!(format_after(false), TextureFormat::LinearArgb8888);
}

#[test]
fn fsd_unreadable_image_is_left_out_and_reported() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new()
        .entry(COVER, b"not an image at all")
        .entry(BACKGROUND, &png(10, 10, GREEN))
        .build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert!(is_empty(app, AssetSlot::Cover));
    assert_eq!(pixels(app, AssetSlot::Background), solid(10, 10, GREEN));
    assert_eq!(app.page(), Page::Background);
    assert_eq!(
        app.modal(),
        Some(&message(
            "Some images could not be read",
            "Cover in game.assets could not be decoded and was left out: the file is not an image format the editor reads"
        ))
    );
}

#[test]
fn fsd_not_enough_space_fills_then_warns() {
    let folder = TempDir::new().unwrap();
    let nineteen: Vec<(AssetSlot, [u8; 4])> =
        (1..=19u8).map(|number| (shot(number), GREY)).collect();
    let existing = write(&folder, "SS1.asset", &asset_bytes(&nineteen));
    let fsd = FsdBuilder::new()
        .screenshot(&png(20, 12, RED))
        .screenshot(&png(20, 12, GREEN))
        .screenshot(&png(20, 12, BLUE))
        .build();
    let fsd = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();
    load(&mut harness, &[&existing]);

    load(&mut harness, &[&fsd]);

    let app = harness.app();
    assert_eq!(app.pages().screenshots.screenshot_count(), 20);
    assert_eq!(pixels(app, shot(20)), solid(20, 12, RED));
    assert_eq!(
        app.modal(),
        Some(&message(
            "Not enough space",
            "Not enough space to fit all screenshots, please clear current screenshots and load the FSD asset again..."
        ))
    );
    assert_eq!(app.page(), Page::Screenshots);
}

#[test]
fn fsd_load_opens_first_loaded_page() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new()
        .entry(BANNER, &png(40, 10, YELLOW))
        .entry(BACKGROUND, &png(30, 20, GREEN))
        .entry(SCREENSHOT, &png(20, 12, RED))
        .build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);

    load(&mut harness, &[&path]);

    assert_eq!(harness.app().page(), Page::Background);
}

#[test]
fn command_line_loads_every_file_by_magic() {
    let folder = TempDir::new().unwrap();
    let asset = write(&folder, "x.png", &asset_bytes(&[(AssetSlot::Cover, RED)]));
    let fsd = write(&folder, "y.asset", &full_fsd());
    let mut harness = AppHarness::new();

    load(&mut harness, &[&asset, &fsd]);

    let app = harness.app();
    assert_eq!(app.modal(), None);
    assert_eq!(pixels(app, AssetSlot::Background), solid(30, 20, GREEN));
    assert_eq!(pixels(app, AssetSlot::Cover), solid(100, 50, RED));
    assert_eq!(
        app.status(),
        &StatusLine::info("Loaded y.asset · 5 of 5 asset types present")
    );
}

#[test]
fn unrecognised_file_is_reported() {
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "notes.asset", b"just some text, no magic");
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "Could not load files",
            "notes.asset: not an Aurora .asset or FSD .assets file\n\nDetails are in error.log."
        ))
    );
    assert!(
        harness
            .error_log_text()
            .contains("not an Aurora .asset or FSD .assets file")
    );
}

#[test]
fn short_file_is_unrecognised() {
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "tiny.asset", b"RXE");
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "Could not load files",
            "tiny.asset: not an Aurora .asset or FSD .assets file\n\nDetails are in error.log."
        ))
    );
}

#[test]
fn truncated_asset_file_reports_its_error() {
    let folder = TempDir::new().unwrap();
    let path = write(&folder, "cut.asset", b"RXEA0123456789");
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "Could not load files",
            "cut.asset: asset file is 14 bytes, shorter than the 2048-byte header\n\nDetails are in error.log."
        ))
    );
}

#[test]
fn missing_command_line_file_is_reported() {
    let folder = TempDir::new().unwrap();
    let missing = folder.path().join("missing.asset");
    let mut harness = AppHarness::new();

    load(&mut harness, &[&missing]);

    assert_eq!(
        harness.app().modal(),
        Some(&message(
            "Could not load files",
            "missing.asset: file not found\n\nDetails are in error.log."
        ))
    );
    assert!(harness.error_log_text().contains("file not found"));
}

#[test]
fn load_problems_share_one_dialog() {
    let folder = TempDir::new().unwrap();
    let bad = write(&folder, "bad.asset", b"nothing useful here");
    let missing = folder.path().join("missing.asset");
    let good = write(
        &folder,
        "GC4D5307E6.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let mut harness = AppHarness::new();

    load(&mut harness, &[&bad, &good, &missing]);

    let app = harness.app();
    assert_eq!(
        app.modal(),
        Some(&message(
            "Could not load files",
            "bad.asset: not an Aurora .asset or FSD .assets file\nmissing.asset: file not found\n\nDetails are in error.log."
        ))
    );
    app.answer_modal(ModalAnswer::Accept);
    assert_eq!(app.modal(), None);
    assert_eq!(pixels(app, AssetSlot::Cover), solid(8, 8, RED));
}

#[test]
fn new_asset_set_resets_pages_and_keeps_game() {
    let folder = TempDir::new().unwrap();
    let file = asset_bytes(&[(AssetSlot::Cover, RED), (AssetSlot::Background, GREEN)]);
    let path = write(&folder, "GC4D5307E6.asset", &file);
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo_3()));
    load(&mut harness, &[&path]);

    harness.app().new_asset_set();

    let app = harness.app();
    assert!(is_empty(app, AssetSlot::Cover));
    assert_eq!(app.pages().present_types(), 0);
    assert_eq!(app.current_game(), Some(&halo_3()));
    assert_eq!(app.status(), &StatusLine::ready());
}

#[test]
fn loading_status_counts_present_types() {
    let folder = TempDir::new().unwrap();
    let cover = write(
        &folder,
        "GC1.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let icon_banner = write(
        &folder,
        "GL1.asset",
        &asset_bytes(&[(AssetSlot::Icon, GREEN), (AssetSlot::Banner, BLUE)]),
    );
    let mut harness = AppHarness::new();

    load(&mut harness, &[&cover]);
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Loaded GC1.asset · 1 of 5 asset types present")
    );

    load(&mut harness, &[&icon_banner]);
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Loaded GL1.asset · 3 of 5 asset types present")
    );
}

#[test]
fn loading_locks_the_editor_until_done() {
    let folder = TempDir::new().unwrap();
    let path = write(
        &folder,
        "GC1.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let mut harness = AppHarness::new();

    harness.app().open_paths(vec![path]);
    assert!(harness.app().locks().editor);
    assert!(harness.app().locks().save_local);

    harness.run_jobs();
    assert_eq!(harness.app().locks(), Locks::default());
}

#[test]
fn non_ascii_paths_load() {
    let folder = TempDir::new().unwrap();
    let game_folder = folder.path().join("ゲーム");
    fs::create_dir(&game_folder).unwrap();
    let path = game_folder.join("カバー.asset");
    fs::write(&path, asset_bytes(&[(AssetSlot::Cover, RED)])).unwrap();
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    assert_eq!(pixels(harness.app(), AssetSlot::Cover), solid(8, 8, RED));
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Loaded カバー.asset · 1 of 5 asset types present")
    );
}

#[test]
fn several_empty_files_share_one_dialog() {
    let folder = TempDir::new().unwrap();
    let empty = AssetFile::new().to_bytes();
    let first = write(&folder, "GC1.asset", &empty);
    let second = write(&folder, "BK1.asset", &empty);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&first, &second]);

    let app = harness.app();
    assert_eq!(
        app.modal(),
        Some(&message(
            "Could not load files",
            "GC1.asset: doesn't contain any assets
BK1.asset: doesn't contain any assets

Details are in error.log."
        ))
    );
    app.answer_modal(ModalAnswer::Accept);
    assert_eq!(app.modal(), None);
}

#[test]
fn fsd_with_only_unreadable_images_does_not_report_loaded() {
    let folder = TempDir::new().unwrap();
    let fsd = FsdBuilder::new().entry(COVER, b"not an image").build();
    let path = write(&folder, "game.assets", &fsd);
    let mut harness = AppHarness::new();

    load(&mut harness, &[&path]);

    let app = harness.app();
    assert_eq!(app.status(), &StatusLine::ready());
    assert_eq!(app.page(), Page::Cover);
    assert_eq!(app.pages().present_types(), 0);
    assert!(matches!(
        app.modal(),
        Some(Modal::Message { title, .. }) if title == "Some images could not be read"
    ));
}

#[test]
fn new_asset_set_during_a_load_is_refused() {
    let folder = TempDir::new().unwrap();
    let path = write(
        &folder,
        "GC1.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let mut harness = AppHarness::new();
    harness.app().open_paths(vec![path]);

    harness.app().new_asset_set();
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Busy, try again when the current work finishes")
    );
    harness.run_jobs();

    assert_eq!(pixels(harness.app(), AssetSlot::Cover), solid(8, 8, RED));
}

#[test]
fn opening_during_a_load_is_refused() {
    let folder = TempDir::new().unwrap();
    let first = write(
        &folder,
        "GC1.asset",
        &asset_bytes(&[(AssetSlot::Cover, RED)]),
    );
    let second = write(
        &folder,
        "BK1.asset",
        &asset_bytes(&[(AssetSlot::Background, GREEN)]),
    );
    let mut harness = AppHarness::new();
    harness.app().open_paths(vec![first]);

    harness.app().open_paths(vec![second]);
    harness.app().open_files_with_dialog();
    assert_eq!(harness.queued_jobs(), 1);
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Busy, try again when the current work finishes")
    );
    harness.run_jobs();

    assert!(is_empty(harness.app(), AssetSlot::Background));
    assert!(harness.dialogs().requests().is_empty());
}
