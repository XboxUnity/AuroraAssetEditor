use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use aurora_asset::{AssetFile, AssetKind, AssetSlot, Compression, RgbaImage, ScreenshotNumber};
use aurora_asset_editor::app::{App, CurrentGame, DISCARD_LABEL, DISCARD_TITLE, KindChoice, Page};
use aurora_asset_editor::dialogs::{ChoiceId, ConfirmedAction, Modal, ModalAnswer, ModalField};
use aurora_asset_editor::jobs::{ConsoleOutcome, JobOutcome, TransferReport, UploadPurpose};
use aurora_asset_editor::status::StatusLine;
use aurora_console::{FtpSettings, GameIds, HexId};
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
    harness.app().answer_modal(ModalAnswer::Accept);

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

/// What Cancel must leave exactly as it was.
#[derive(Debug, PartialEq)]
struct EditorState {
    bytes: Vec<Vec<u8>>,
    unsaved: Vec<AssetKind>,
    page: Page,
    status: StatusLine,
    toasts: usize,
}

fn editor_state(app: &mut App) -> EditorState {
    EditorState {
        bytes: AssetKind::ALL
            .into_iter()
            .map(|kind| app.page_bytes(kind))
            .collect(),
        unsaved: unsaved(app),
        page: app.page(),
        status: app.status().clone(),
        toasts: app.toasts().iter().count(),
    }
}

/// The text of the Discard question on screen, after checking it is drawn as the app's other confirmations.
fn discard_question(app: &App, expected_action: &ConfirmedAction) -> String {
    match app.modal() {
        Some(Modal::Confirm {
            title,
            text,
            confirm_label,
            danger,
            cancel_is_default,
            action,
        }) => {
            assert_eq!(title, DISCARD_TITLE);
            assert_eq!(title, "Unsaved changes");
            assert_eq!(confirm_label, DISCARD_LABEL);
            assert_eq!(confirm_label, "Discard");
            assert!(*danger, "Discard is drawn in the danger colour");
            assert!(*cancel_is_default, "Cancel is the default");
            assert_eq!(action, expected_action);
            text.clone()
        }
        other => panic!("expected the Discard question, got {other:?}"),
    }
}

/// Cover and Background edited, the other pages as opened.
fn edited_harness(folder: &TempDir) -> AppHarness {
    let mut harness = opened_harness(folder);
    let art = png(folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    replace(&mut harness, AssetSlot::Background, &art);
    harness
}

#[test]
fn opening_a_file_asks_only_about_the_page_it_replaces() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    let cover_bytes = asset_bytes(&[AssetSlot::Cover]);
    let cover = write(&folder, "GC.asset", &cover_bytes);
    let before = editor_state(&mut harness.app());

    open(&mut harness, vec![cover.clone()]);
    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::DiscardChanges),
        "Discard unsaved changes to Cover?"
    );
    harness.app().answer_modal(ModalAnswer::Cancel);

    assert_eq!(editor_state(&mut harness.app()), before);
    assert_eq!(harness.app().modal(), None);

    open(&mut harness, vec![cover]);
    harness.app().answer_modal(ModalAnswer::Accept);

    let app = &mut *harness.app();
    assert_eq!(app.modal(), None);
    assert_eq!(app.page(), Page::Cover);
    assert_eq!(unsaved(app), vec![AssetKind::Background]);
    assert_eq!(app.page_bytes(AssetKind::Cover), cover_bytes);
    assert_eq!(
        app.status().text,
        "Loaded GC.asset · 5 of 5 asset types present"
    );
}

#[test]
fn nothing_is_asked_when_the_replaced_pages_are_saved() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    let screenshots = write(&folder, "SS.asset", &asset_bytes(&[shot(4)]));

    open(&mut harness, vec![screenshots]);

    assert_eq!(harness.app().modal(), None);
    assert_eq!(harness.app().page(), Page::Screenshots);
    assert_eq!(
        unsaved(&harness.app()),
        vec![AssetKind::Cover, AssetKind::Background]
    );
}

#[test]
fn opening_several_files_asks_once_for_every_page_they_replace() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    harness.app().move_screenshot(number(1), number(5));
    let files = page_files(&folder);
    let before = editor_state(&mut harness.app());

    open(&mut harness, files);
    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::DiscardChanges),
        "Discard unsaved changes to Cover, Background and Screenshots?"
    );
    harness.app().answer_modal(ModalAnswer::Cancel);

    assert_eq!(editor_state(&mut harness.app()), before);
    assert_eq!(
        harness.app().modal(),
        None,
        "one question for the whole open"
    );
}

#[test]
fn an_fsd_file_asks_only_about_the_pages_whose_images_it_replaces() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    harness.app().move_screenshot(number(1), number(5));
    harness.app().remove_image(AssetSlot::Icon);
    let fsd = FsdBuilder::new()
        .entry(COVER, &png_bytes(RED))
        .screenshot(&png_bytes(RED))
        .build();
    let path = write(&folder, "game.assets", &fsd);

    open(&mut harness, vec![path]);

    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::DiscardChanges),
        "Discard unsaved changes to Cover?",
        "the FSD screenshots only fill free positions"
    );
    harness.app().answer_modal(ModalAnswer::Accept);
    assert_eq!(harness.app().pages().screenshots.screenshot_count(), 3);
}

#[test]
fn cancel_still_reports_the_files_that_could_not_be_read() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    let cover = write(&folder, "GC.asset", &asset_bytes(&[AssetSlot::Cover]));
    let missing = folder.path().join("missing.asset");
    let before = editor_state(&mut harness.app());

    open(&mut harness, vec![cover, missing]);
    harness.app().answer_modal(ModalAnswer::Cancel);

    match harness.app().modal() {
        Some(Modal::Message { title, text }) => {
            assert_eq!(title, "Could not load files");
            assert!(text.contains("missing.asset: file not found"), "{text}");
        }
        other => panic!("expected the load problems, got {other:?}"),
    }
    assert_eq!(editor_state(&mut harness.app()), before);
}

#[test]
fn new_asset_set_asks_about_every_unsaved_page() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    harness.app().move_screenshot(number(1), number(5));
    let before = editor_state(&mut harness.app());

    harness.app().new_asset_set();
    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::DiscardChanges),
        "Discard unsaved changes to Cover, Background and Screenshots?"
    );
    harness.app().answer_modal(ModalAnswer::Cancel);
    assert_eq!(editor_state(&mut harness.app()), before);

    harness.app().new_asset_set();
    harness.app().answer_modal(ModalAnswer::Accept);
    let app = &mut *harness.app();
    assert_eq!(app.pages().present_types(), 0);
    assert_eq!(unsaved(app), Vec::new());
    assert_eq!(app.status(), &StatusLine::ready());
}

#[test]
fn closing_with_unsaved_changes_asks_once_and_discard_lets_it_close() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);

    assert!(!harness.app().request_close());
    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::CloseWindow),
        "Discard unsaved changes to Cover and Background?"
    );
    assert!(!harness.app().request_close(), "a second request waits too");
    harness.app().answer_modal(ModalAnswer::Cancel);
    assert_eq!(harness.app().modal(), None, "the question was asked once");
    assert!(!harness.app().close_approved());

    assert!(!harness.app().request_close());
    harness.app().answer_modal(ModalAnswer::Accept);

    assert!(harness.app().close_approved());
    assert!(harness.app().request_close());
    assert_eq!(
        unsaved(&harness.app()),
        vec![AssetKind::Cover, AssetKind::Background],
        "closing touches no page"
    );
}

#[test]
fn closing_without_unsaved_changes_does_not_ask() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);

    assert!(harness.app().request_close());
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn a_queued_close_question_with_nothing_left_to_lose_lets_the_window_close() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    replace(&mut harness, AssetSlot::Cover, &png(&folder, "art.png"));
    let cover = write(&folder, "GC.asset", &asset_bytes(&[AssetSlot::Cover]));
    open(&mut harness, vec![cover]);
    assert!(
        !harness.app().request_close(),
        "the close waits behind the open"
    );

    harness.app().answer_modal(ModalAnswer::Accept);

    assert_eq!(unsaved(&harness.app()), Vec::new());
    assert_eq!(
        harness.app().modal(),
        None,
        "the close question is not asked"
    );
    assert!(harness.app().close_approved());
    assert!(harness.app().request_close());
}

#[test]
fn a_queued_question_names_the_pages_still_unsaved_when_it_comes_up() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    let cover = write(&folder, "GC.asset", &asset_bytes(&[AssetSlot::Cover]));
    open(&mut harness, vec![cover]);
    assert!(!harness.app().request_close());

    harness.app().answer_modal(ModalAnswer::Accept);

    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::CloseWindow),
        "Discard unsaved changes to Background?"
    );
    assert!(!harness.app().close_approved());
}

#[test]
fn a_queued_replacement_with_nothing_left_to_lose_runs_without_asking() {
    let folder = TempDir::new().unwrap();
    let mut harness = edited_harness(&folder);
    let cover_bytes = asset_bytes(&[AssetSlot::Cover]);
    let cover = write(&folder, "GC.asset", &cover_bytes);
    harness.app().new_asset_set();
    open(&mut harness, vec![cover]);

    harness.app().answer_modal(ModalAnswer::Accept);

    let app = &mut *harness.app();
    assert_eq!(app.modal(), None, "the open finds nothing left to lose");
    assert_eq!(app.page_bytes(AssetKind::Cover), cover_bytes);
    assert_eq!(app.pages().present_types(), 1);
    assert_eq!(unsaved(app), Vec::new());
}

#[test]
fn an_import_landing_under_the_close_question_adds_its_page_to_it() {
    let folder = TempDir::new().unwrap();
    let mut harness = opened_harness(&folder);
    let art = png(&folder, "art.png");
    replace(&mut harness, AssetSlot::Cover, &art);
    harness.dialogs().push_files(vec![art]);
    harness.app().replace_image(AssetSlot::Background);
    assert!(!harness.app().request_close());
    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::CloseWindow),
        "Discard unsaved changes to Cover?"
    );

    harness.run_jobs();

    assert_eq!(
        discard_question(&harness.app(), &ConfirmedAction::CloseWindow),
        "Discard unsaved changes to Cover and Background?"
    );
    assert!(!harness.app().close_approved());
}

#[test]
fn an_upload_that_saves_every_page_under_the_close_question_lets_the_window_close() {
    let folder = TempDir::new().unwrap();
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: "192.168.1.50".to_owned(),
        port: "21".to_owned(),
        ..FtpSettings::default()
    });
    open(&mut harness, page_files(&folder));
    replace(&mut harness, AssetSlot::Cover, &png(&folder, "art.png"));
    let ids = GameIds {
        title_id: HexId::parse(TITLE_ID).unwrap(),
        database_id: HexId::from_database_value(18),
    };
    harness.app().set_current_game(Some(CurrentGame {
        title: "Halo 3".to_owned(),
        ids: ids.clone(),
    }));
    harness
        .app()
        .upload_current(KindChoice::One(AssetKind::Cover));
    assert!(harness.take_next_job().is_some(), "the upload is queued");
    assert!(!harness.app().request_close());

    harness
        .app()
        .on_job_outcome(JobOutcome::Console(ConsoleOutcome::Uploaded {
            report: TransferReport {
                ids,
                done: vec![AssetKind::Cover],
                failure: None,
            },
            purpose: UploadPurpose::UploadCurrent,
        }));

    assert_eq!(harness.app().modal(), None, "nothing is left to lose");
    assert!(harness.app().close_approved());
}
