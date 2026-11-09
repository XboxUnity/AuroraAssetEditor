use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetFile, AssetKind, AssetSlot, Compression, RgbaImage, ScreenshotNumber};
use aurora_asset_editor::app::{App, Page};
use aurora_asset_editor::dialogs::{ChoiceId, ModalAnswer, ModalField};
use aurora_console::HexId;
use editor_test_support::{AppHarness, COVER, FsdBuilder, ICON, asset_file_with};
use image::{ImageFormat, Rgba};
use tempfile::TempDir;

const TITLE_ID: &str = "4D5307E6";
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn solid(color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(8, 8, Rgba(color))
}

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    solid(color)
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("a small image encodes as PNG");
    bytes.into_inner()
}

fn write(folder: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = folder.path().join(name);
    fs::write(&path, bytes).expect("the temp folder is writable");
    path
}

fn png(folder: &TempDir, name: &str) -> PathBuf {
    write(folder, name, &png_bytes(GREEN))
}

fn shot(position: u8) -> AssetSlot {
    AssetSlot::Screenshot(number(position))
}

fn number(position: u8) -> ScreenshotNumber {
    ScreenshotNumber::new(position).expect("positions 1 to 20 exist")
}

fn asset_bytes(slots: &[AssetSlot]) -> Vec<u8> {
    let entries: Vec<_> = slots
        .iter()
        .map(|slot| (*slot, solid(RED), Compression::None))
        .collect();
    asset_file_with(&entries).to_bytes()
}

fn kind_slots(kind: AssetKind) -> Vec<AssetSlot> {
    match kind {
        AssetKind::Cover => vec![AssetSlot::Cover],
        AssetKind::Background => vec![AssetSlot::Background],
        AssetKind::IconBanner => vec![AssetSlot::Icon, AssetSlot::Banner],
        AssetKind::Screenshots => vec![shot(1), shot(3)],
    }
}

fn asset_file_name(kind: AssetKind) -> String {
    aurora_console::asset_file_name(kind, &HexId::parse(TITLE_ID).unwrap())
}

/// One `.asset` file per kind, each with content.
fn page_files(folder: &TempDir) -> Vec<PathBuf> {
    AssetKind::ALL
        .into_iter()
        .map(|kind| {
            write(
                folder,
                &asset_file_name(kind),
                &asset_bytes(&kind_slots(kind)),
            )
        })
        .collect()
}

fn open(harness: &mut AppHarness, paths: Vec<PathBuf>) {
    harness.app().open_paths(paths);
    harness.run_jobs();
}

/// Every page holds the content of a file just opened.
fn opened_harness(folder: &TempDir) -> AppHarness {
    let mut harness = AppHarness::new();
    open(&mut harness, page_files(folder));
    assert_eq!(
        unsaved(&harness.app()),
        Vec::new(),
        "opened pages are saved"
    );
    harness
}

fn unsaved(app: &App) -> Vec<AssetKind> {
    app.pages().unsaved_among(&AssetKind::ALL)
}

fn replace(harness: &mut AppHarness, slot: AssetSlot, path: &Path) {
    harness.dialogs().push_files(vec![path.to_path_buf()]);
    harness.app().replace_image(slot);
    harness.run_jobs();
}

#[test]
fn replace_marks_only_its_page_changed() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");

    replace(&mut harness, AssetSlot::Cover, &art);
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Cover]);

    replace(&mut harness, AssetSlot::Banner, &art);
    assert_eq!(
        unsaved(&harness.app()),
        vec![AssetKind::Cover, AssetKind::IconBanner]
    );
}

#[test]
fn add_marks_the_screenshots_changed() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    harness.dialogs().push_files(vec![png(&folder, "art.png")]);

    harness.app().add_screenshots();
    harness.run_jobs();

    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Screenshots]);
}

#[test]
fn remove_marks_its_page_changed_and_removing_nothing_does_not() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);

    harness.app().remove_image(shot(2));
    assert_eq!(unsaved(&harness.app()), Vec::new(), "position 2 was empty");

    harness.app().remove_image(AssetSlot::Background);
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Background]);
}

#[test]
fn reorder_marks_the_screenshots_changed_and_a_drop_in_place_does_not() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);

    harness.app().move_screenshot(number(1), number(1));
    assert_eq!(unsaved(&harness.app()), Vec::new());

    harness.app().move_screenshot(number(1), number(2));
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Screenshots]);
}

#[test]
fn image_drops_mark_the_page_they_land_on_changed() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");

    harness.app().show_page(Page::Background);
    harness.app().files_dropped(vec![art.clone()]);
    harness.run_jobs();
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Background]);

    harness.app().show_page(Page::IconBanner);
    harness.app().files_dropped(vec![art.clone()]);
    harness
        .app()
        .answer_modal(ModalAnswer::Choice(ChoiceId::Icon));
    harness.run_jobs();
    assert_eq!(
        unsaved(&harness.app()),
        vec![AssetKind::Background, AssetKind::IconBanner]
    );

    harness.app().show_page(Page::Screenshots);
    harness.app().select_screenshot(number(2));
    harness.app().files_dropped(vec![art]);
    harness.run_jobs();
    assert_eq!(
        unsaved(&harness.app()),
        vec![
            AssetKind::Background,
            AssetKind::IconBanner,
            AssetKind::Screenshots
        ]
    );
}

#[test]
fn an_fsd_file_marks_the_pages_it_put_images_on_changed() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    let fsd = FsdBuilder::new()
        .entry(COVER, &png_bytes(RED))
        .entry(ICON, &png_bytes(GREEN))
        .screenshot(&png_bytes(RED))
        .build();
    let path = write(&folder, "game.assets", &fsd);

    open(&mut harness, vec![path]);

    assert_eq!(
        unsaved(&harness.app()),
        vec![
            AssetKind::Cover,
            AssetKind::IconBanner,
            AssetKind::Screenshots
        ],
        "an FSD file is converted, so its images exist in no asset file yet"
    );
}

#[test]
fn opening_a_file_leaves_its_page_saved() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    replace(&mut harness, AssetSlot::Background, &art);
    let cover = write(&folder, "GC.asset", &asset_bytes(&[AssetSlot::Cover]));

    open(&mut harness, vec![cover]);

    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Background]);
}

#[test]
fn save_clears_only_its_page() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    replace(&mut harness, AssetSlot::Background, &art);
    harness
        .dialogs()
        .push_save(folder.path().join("saved-cover.asset"));

    harness.app().save_kind(AssetKind::Cover);

    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Background]);
}

#[test]
fn a_cancelled_or_failed_save_keeps_the_changes() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    replace(&mut harness, AssetSlot::Cover, &png(&folder, "art.png"));

    harness.dialogs().push_cancel();
    harness.app().save_kind(AssetKind::Cover);
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Cover]);

    let missing_folder = folder.path().join("missing").join("cover.asset");
    harness.dialogs().push_save(missing_folder);
    harness.app().save_kind(AssetKind::Cover);
    assert!(harness.app().modal().is_some(), "the failure is shown");
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Cover]);
}

#[test]
fn a_failed_screenshots_save_keeps_the_compacted_page_changed() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    assert!(
        !harness.app().pages().screenshots.screenshots_are_compact(),
        "positions 1 and 3 leave a gap"
    );

    harness
        .dialogs()
        .push_save(folder.path().join("missing").join("ss.asset"));
    harness.app().save_kind(AssetKind::Screenshots);

    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Screenshots]);
}

#[test]
fn save_all_clears_the_pages_it_wrote() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    harness.app().remove_image(AssetSlot::Background);
    let target = TempDir::new().unwrap();
    // An empty page never overwrites an existing file, so the removed background stays unsaved.
    fs::write(
        target.path().join(asset_file_name(AssetKind::Background)),
        AssetFile::new().to_bytes(),
    )
    .unwrap();

    harness.dialogs().push_folder(target.path().to_path_buf());
    harness.app().save_all_locally();
    harness
        .app()
        .edit_modal_field(ModalField::TitleId, TITLE_ID);
    harness.app().answer_modal(ModalAnswer::Accept);

    assert_eq!(unsaved(&harness.app()), vec![AssetKind::Background]);
}

#[test]
fn a_failed_save_all_clears_only_the_pages_written_before_it() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    replace(&mut harness, AssetSlot::Icon, &art);
    let target = TempDir::new().unwrap();
    // A folder where the icon & banner file should go makes that write fail.
    fs::create_dir(target.path().join(asset_file_name(AssetKind::IconBanner))).unwrap();

    harness.dialogs().push_folder(target.path().to_path_buf());
    harness.app().save_all_locally();
    harness
        .app()
        .edit_modal_field(ModalField::TitleId, TITLE_ID);
    harness.app().answer_modal(ModalAnswer::Accept);

    assert!(harness.app().modal().is_some(), "the failure is shown");
    assert_eq!(unsaved(&harness.app()), vec![AssetKind::IconBanner]);
}

#[test]
fn new_asset_set_leaves_every_page_saved() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    open(&mut harness, page_files(&folder));

    harness.app().new_asset_set();

    assert_eq!(unsaved(&harness.app()), Vec::new());
}
