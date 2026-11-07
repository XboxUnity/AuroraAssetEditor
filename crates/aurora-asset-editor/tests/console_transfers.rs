use std::cell::Cell;
use std::path::PathBuf;

use aurora_asset::{
    AssetFile, AssetKind, AssetSlot, Compression, RgbaImage, ScreenshotNumber, decode,
};
use aurora_asset_editor::app::Page;
use aurora_asset_editor::app::{
    BUSY_TEXT, CONNECTION_FAILED_TITLE, CurrentGame, EMPTY_UPLOAD_TITLE, KindChoice,
    NO_CONSOLE_SET_UP_REASON, REMOVE_CONFIRM_TITLE, SELECT_A_GAME_REASON, UPLOAD_TARGET_TITLE,
};
use aurora_asset_editor::console::PresenceCell;
use aurora_asset_editor::dialogs::{Modal, ModalAnswer, ModalField};
use aurora_asset_editor::editor::SlotPreview;
use aurora_asset_editor::status::Severity;
use aurora_console::{
    AssetPresence, FtpSettings, GameIds, HexId, asset_file_name, asset_folder, read_content_db,
};
use editor_test_support::{AppHarness, OutcomeStage, asset_file_with};
use test_servers::{FakeConsole, FakeConsoleConfig};

const GAME_TITLE: &str = "Halo";

fn ids(title_id: &str, database_id: &str) -> GameIds {
    GameIds {
        title_id: HexId::parse(title_id).unwrap(),
        database_id: HexId::parse(database_id).unwrap(),
    }
}

fn halo() -> GameIds {
    ids("4D5307E6", "00000012")
}

fn solid(shade: u8) -> RgbaImage {
    RgbaImage::from_pixel(8, 8, image::Rgba([shade, 40, 90, 255]))
}

fn cover_file(shade: u8) -> AssetFile {
    asset_file_with(&[(AssetSlot::Cover, solid(shade), Compression::None)])
}

fn background_file() -> AssetFile {
    asset_file_with(&[(AssetSlot::Background, solid(7), Compression::None)])
}

fn icon_banner_file() -> AssetFile {
    asset_file_with(&[
        (AssetSlot::Icon, solid(11), Compression::None),
        (AssetSlot::Banner, solid(12), Compression::None),
    ])
}

fn screenshot_slot(number: u8) -> AssetSlot {
    AssetSlot::Screenshot(ScreenshotNumber::new(number).unwrap())
}

fn console_path(ids: &GameIds, kind: AssetKind) -> String {
    format!(
        "{}/{}",
        asset_folder(ids),
        asset_file_name(kind, &ids.title_id)
    )
}

fn empty_asset_bytes() -> Vec<u8> {
    AssetFile::new().to_bytes()
}

fn harness_with_empty_console() -> (AppHarness, FakeConsole) {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    (harness, console)
}

fn select_halo(harness: &mut AppHarness) {
    harness.app().set_current_game(Some(CurrentGame {
        title: GAME_TITLE.to_owned(),
        ids: halo(),
    }));
}

fn put_on_console(console: &FakeConsole, kind: AssetKind, file: &mut AssetFile) {
    console.add_file(&console_path(&halo(), kind), &file.to_bytes());
}

/// Loads `file` into the editor the way Open does.
fn load_into_editor(harness: &mut AppHarness, name: &str, file: &mut AssetFile) {
    let path = harness.config_dir().join(name);
    std::fs::write(&path, file.to_bytes()).expect("the temp folder is writable");
    harness.app().open_paths(vec![path]);
    harness.run_jobs();
}

fn confirm_text(harness: &mut AppHarness, expected_title: &str) -> String {
    match harness.app().modal() {
        Some(Modal::Confirm { title, text, .. }) => {
            assert_eq!(title, expected_title);
            text.clone()
        }
        other => panic!("expected a confirmation dialog, got {other:?}"),
    }
}

fn retrieved_files(console: &FakeConsole) -> Vec<String> {
    console
        .commands()
        .into_iter()
        .filter_map(|command| command.strip_prefix("RETR ").map(str::to_owned))
        .collect()
}

#[test]
fn download_cover_loads_and_opens_cover_page() {
    let (mut harness, console) = harness_with_empty_console();
    let mut cover = cover_file(10);
    put_on_console(&console, AssetKind::Cover, &mut cover);
    select_halo(&mut harness);
    harness.app().show_page(Page::Console);

    harness.app().download(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();

    let app = harness.app();
    assert_eq!(app.page(), Page::Cover);
    let expected = decode(cover.get(AssetSlot::Cover).unwrap()).unwrap();
    assert_eq!(
        app.pages().cover.preview(AssetSlot::Cover),
        &SlotPreview::Pixels(expected)
    );
    assert_eq!(app.status().text, "Downloaded cover from the console");
}

#[test]
fn missing_console_asset_shows_message() {
    let (mut harness, _console) = harness_with_empty_console();
    select_halo(&mut harness);
    harness.app().show_page(Page::Console);

    harness.app().download(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();

    let app = harness.app();
    assert_eq!(app.status().text, "No cover asset on console");
    assert_eq!(app.status().severity, Severity::Info);
    assert!(!app.pages().cover.has_content());
    assert_eq!(app.page(), Page::Console);
}

#[test]
fn empty_console_asset_counts_as_missing() {
    let (mut harness, console) = harness_with_empty_console();
    console.add_file(
        &console_path(&halo(), AssetKind::Cover),
        &empty_asset_bytes(),
    );
    load_into_editor(&mut harness, "local-cover.asset", &mut cover_file(30));
    select_halo(&mut harness);
    harness.app().show_page(Page::Console);

    harness.app().download(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();

    let app = harness.app();
    assert_eq!(app.status().text, "No cover asset on console");
    assert!(app.pages().cover.has_content());
    assert_eq!(app.page(), Page::Console);
}

#[test]
fn download_all_runs_gc_bk_gl_ss_and_skips_missing() {
    let (mut harness, console) = harness_with_empty_console();
    put_on_console(&console, AssetKind::IconBanner, &mut icon_banner_file());
    put_on_console(&console, AssetKind::Background, &mut background_file());
    put_on_console(&console, AssetKind::Cover, &mut cover_file(10));
    select_halo(&mut harness);
    harness.app().show_page(Page::Console);

    harness.app().download(KindChoice::All);
    harness.run_jobs();

    let title_id = halo().title_id.to_string();
    assert_eq!(
        retrieved_files(&console),
        vec![
            format!("GC{title_id}.asset"),
            format!("BK{title_id}.asset"),
            format!("GL{title_id}.asset"),
        ]
    );
    let app = harness.app();
    assert_eq!(
        app.status().text,
        "Downloaded cover, background and icon & banner from the console · No screenshots asset on console"
    );
    assert_eq!(app.page(), Page::Console);
    assert!(app.pages().icon_banner.has_content());
}

#[test]
fn download_all_stops_at_first_error() {
    let (mut harness, console) = harness_with_empty_console();
    put_on_console(&console, AssetKind::Cover, &mut cover_file(10));
    put_on_console(&console, AssetKind::Background, &mut background_file());
    put_on_console(&console, AssetKind::IconBanner, &mut icon_banner_file());
    console.truncate_download(&console_path(&halo(), AssetKind::Background), 10);
    select_halo(&mut harness);

    harness.app().download(KindChoice::All);
    harness.run_jobs();

    let title_id = halo().title_id.to_string();
    assert!(
        !retrieved_files(&console).contains(&format!("GL{title_id}.asset")),
        "the icon & banner file was requested after the failure"
    );
    let app = harness.app();
    assert_eq!(app.status().severity, Severity::Error);
    assert!(
        app.status()
            .text
            .starts_with("Could not download background: ")
    );
    assert_eq!(app.modal(), None);
    assert!(app.pages().cover.has_content());
    assert!(!app.pages().icon_banner.has_content());
}

#[test]
fn open_in_editor_opens_first_loaded_page() {
    let (mut harness, console) = harness_with_empty_console();
    put_on_console(&console, AssetKind::IconBanner, &mut icon_banner_file());
    put_on_console(&console, AssetKind::Background, &mut background_file());
    select_halo(&mut harness);
    harness.app().show_page(Page::Console);

    harness.app().open_in_editor();
    harness.run_jobs();

    assert_eq!(harness.app().page(), Page::Background);
}

#[test]
fn upload_cover_sends_page_bytes_unchanged() {
    let (mut harness, console) = harness_with_empty_console();
    console.add_dir(&asset_folder(&halo()));
    let mut cover = cover_file(10);
    load_into_editor(&mut harness, "cover.asset", &mut cover);
    select_halo(&mut harness);

    harness
        .app()
        .upload_current(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();

    assert_eq!(
        console.file(&console_path(&halo(), AssetKind::Cover)),
        Some(cover.to_bytes())
    );
    assert_eq!(harness.app().status().text, "Uploaded cover to the console");
}

#[test]
fn upload_screenshots_keeps_gaps() {
    let (mut harness, console) = harness_with_empty_console();
    console.add_dir(&asset_folder(&halo()));
    let mut screenshots = asset_file_with(&[
        (screenshot_slot(2), solid(1), Compression::None),
        (screenshot_slot(5), solid(2), Compression::None),
    ]);
    load_into_editor(&mut harness, "screenshots.asset", &mut screenshots);
    select_halo(&mut harness);

    harness
        .app()
        .upload_current(KindChoice::One(AssetKind::Screenshots));
    harness.run_jobs();

    let bytes = console
        .file(&console_path(&halo(), AssetKind::Screenshots))
        .expect("the screenshots were uploaded");
    let uploaded = AssetFile::parse(&bytes).unwrap();
    let filled: Vec<u8> = ScreenshotNumber::ALL
        .into_iter()
        .filter(|number| uploaded.get(AssetSlot::Screenshot(*number)).is_some())
        .map(ScreenshotNumber::get)
        .collect();
    assert_eq!(filled, vec![2, 5]);
}

#[test]
fn uploading_empty_kinds_asks_first() {
    let (mut harness, console) = harness_with_empty_console();
    console.add_dir(&asset_folder(&halo()));
    load_into_editor(&mut harness, "cover.asset", &mut cover_file(10));
    select_halo(&mut harness);

    harness.app().upload_current(KindChoice::All);
    let text = confirm_text(&mut harness, EMPTY_UPLOAD_TITLE);
    assert_eq!(
        text,
        "Background, icon & banner and screenshots are empty in the editor. Uploading replaces them on the console with an empty asset, which removes them."
    );
    harness.app().answer_modal(ModalAnswer::Cancel);
    harness.run_jobs();
    for kind in AssetKind::ALL {
        assert_eq!(console.file(&console_path(&halo(), kind)), None);
    }

    harness.app().upload_current(KindChoice::All);
    harness.app().answer_modal(ModalAnswer::Accept);
    harness.run_jobs();

    let cover = console
        .file(&console_path(&halo(), AssetKind::Cover))
        .unwrap();
    assert_ne!(cover, empty_asset_bytes());
    for kind in [
        AssetKind::Background,
        AssetKind::IconBanner,
        AssetKind::Screenshots,
    ] {
        assert_eq!(
            console.file(&console_path(&halo(), kind)),
            Some(empty_asset_bytes())
        );
    }
}

#[test]
fn remove_asks_then_uploads_empty_asset() {
    let (mut harness, console) = harness_with_empty_console();
    put_on_console(&console, AssetKind::Cover, &mut cover_file(10));
    load_into_editor(&mut harness, "cover.asset", &mut cover_file(50));
    select_halo(&mut harness);

    harness
        .app()
        .remove_from_console(KindChoice::One(AssetKind::Cover));
    let text = confirm_text(&mut harness, REMOVE_CONFIRM_TITLE);
    assert_eq!(
        text,
        "Remove the cover of Halo (4D5307E6) from the console? Aurora will no longer show it."
    );
    harness.app().answer_modal(ModalAnswer::Accept);
    harness.run_jobs();

    assert_eq!(
        console.file(&console_path(&halo(), AssetKind::Cover)),
        Some(empty_asset_bytes())
    );
    assert!(harness.app().pages().cover.has_content());
    assert_eq!(
        harness.app().status().text,
        "Removed cover from the console"
    );
}

#[test]
fn remove_cancelled_changes_nothing() {
    let (mut harness, console) = harness_with_empty_console();
    let mut cover = cover_file(10);
    put_on_console(&console, AssetKind::Cover, &mut cover);
    select_halo(&mut harness);

    harness.app().remove_from_console(KindChoice::All);
    harness.app().answer_modal(ModalAnswer::Cancel);
    harness.run_jobs();

    assert_eq!(
        console.file(&console_path(&halo(), AssetKind::Cover)),
        Some(cover.to_bytes())
    );
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn save_all_to_console_needs_console_setup() {
    let mut without = AppHarness::new();
    assert_eq!(
        without.app().save_to_console_disabled_reason(),
        Some("No console set up")
    );
    assert_eq!(NO_CONSOLE_SET_UP_REASON, "No console set up");

    let mut with = AppHarness::new();
    with.with_ftp_settings(FtpSettings::default());
    assert_eq!(with.app().save_to_console_disabled_reason(), None);
}

#[test]
fn save_all_to_console_uploads_only_kinds_with_content() {
    let (mut harness, console) = harness_with_empty_console();
    let typed = ids("4d5307e6", "0000001a");
    console.add_dir(&asset_folder(&typed));
    let mut cover = cover_file(10);
    load_into_editor(&mut harness, "cover.asset", &mut cover);

    harness.app().save_all_to_console();
    harness
        .app()
        .edit_modal_field(ModalField::UploadTitleId, "4d5307e6");
    harness
        .app()
        .edit_modal_field(ModalField::UploadDatabaseId, "0000001a");
    harness.app().answer_modal(ModalAnswer::Accept);
    harness.run_jobs();

    assert_eq!(
        console.file("/Game/Data/GameData/4d5307e6_0000001a/GC4d5307e6.asset"),
        Some(cover.to_bytes())
    );
    for kind in [
        AssetKind::Background,
        AssetKind::IconBanner,
        AssetKind::Screenshots,
    ] {
        assert_eq!(console.file(&console_path(&typed, kind)), None);
    }
    assert_eq!(harness.app().status().text, "Uploaded cover to the console");
}

#[test]
fn upload_target_dialog_rules() {
    let (mut harness, _console) = harness_with_empty_console();
    select_halo(&mut harness);

    harness.app().save_all_to_console();
    let Some(Modal::UploadTarget { title, form }) = harness.app().modal() else {
        panic!("expected the upload target dialog");
    };
    assert_eq!(title, UPLOAD_TARGET_TITLE);
    assert_eq!(form.title_id, "4D5307E6");
    assert_eq!(form.database_id, "00000012");

    harness
        .app()
        .edit_modal_field(ModalField::UploadTitleId, "zz4D-53");
    harness
        .app()
        .edit_modal_field(ModalField::UploadDatabaseId, "123456789");
    let Some(Modal::UploadTarget { form, .. }) = harness.app().modal() else {
        panic!("expected the upload target dialog");
    };
    assert_eq!(form.title_id, "4D53");
    assert_eq!(form.database_id, "12345678");
    assert!(!form.is_valid());

    harness.app().answer_modal(ModalAnswer::Accept);
    assert!(matches!(
        harness.app().modal(),
        Some(Modal::UploadTarget { .. })
    ));

    harness
        .app()
        .edit_modal_field(ModalField::UploadTitleId, "4D5307E6");
    harness.app().answer_modal(ModalAnswer::Cancel);
    assert_eq!(harness.app().modal(), None);
}

#[test]
fn connection_failure_shows_old_message() {
    let (mut harness, console) = harness_with_empty_console();
    console.refuse_new_connections();
    select_halo(&mut harness);

    harness.app().download(KindChoice::All);
    harness.run_jobs();

    let Some(Modal::Message { title, text }) = harness.app().modal() else {
        panic!("expected a message dialog");
    };
    assert_eq!(title, CONNECTION_FAILED_TITLE);
    assert!(text.starts_with("FTP Connection could not be established: could not connect to"));
    assert_eq!(harness.app().status().severity, Severity::Error);
}

#[test]
fn missing_folder_upload_is_shown() {
    let (mut harness, _console) = harness_with_empty_console();
    load_into_editor(&mut harness, "cover.asset", &mut cover_file(10));
    select_halo(&mut harness);

    harness
        .app()
        .upload_current(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();

    let app = harness.app();
    assert_eq!(app.modal(), None);
    assert_eq!(app.status().severity, Severity::Error);
    assert!(
        app.status()
            .text
            .contains("the console has no folder /Game/Data/GameData/4D5307E6_00000012"),
        "{}",
        app.status().text
    );
}

#[test]
fn presence_updates_after_upload_and_remove() {
    let typical = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/console/content-db/typical.db"),
    )
    .expect("the sample database is readable");
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical);
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    let games = read_content_db(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/console/content-db/typical.db"),
    )
    .unwrap();
    let game = games[0].clone();
    console.add_dir(&asset_folder(&game.ids));
    harness.app().refresh_games();
    harness.run_jobs();
    harness.app().set_current_game(Some(CurrentGame {
        title: game.title_name.clone(),
        ids: game.ids.clone(),
    }));
    assert_eq!(
        harness.app().console().table.presence(&game.ids),
        PresenceCell::Known(AssetPresence::NONE)
    );
    load_into_editor(&mut harness, "cover.asset", &mut cover_file(10));

    harness
        .app()
        .upload_current(KindChoice::One(AssetKind::Cover));
    harness.run_jobs();
    let PresenceCell::Known(after_upload) = harness.app().console().table.presence(&game.ids)
    else {
        panic!("the presence was not checked again");
    };
    assert!(after_upload.cover);

    harness
        .app()
        .remove_from_console(KindChoice::One(AssetKind::Cover));
    harness.app().answer_modal(ModalAnswer::Accept);
    harness.run_jobs();
    assert_eq!(
        harness.app().console().table.presence(&game.ids),
        PresenceCell::Known(AssetPresence::NONE)
    );
}

#[test]
fn console_actions_need_a_selected_game() {
    let (mut harness, _console) = harness_with_empty_console();
    assert_eq!(
        harness.app().console_actions_disabled_reason(),
        Some("Select a game first")
    );
    assert_eq!(SELECT_A_GAME_REASON, "Select a game first");

    harness.app().download(KindChoice::All);
    assert_eq!(harness.queued_jobs(), 0);
    assert_eq!(harness.app().status().text, SELECT_A_GAME_REASON);

    select_halo(&mut harness);
    assert_eq!(harness.app().console_actions_disabled_reason(), None);
}

#[test]
fn opening_and_dropping_files_during_a_download_is_refused() {
    let (mut harness, console) = harness_with_empty_console();
    put_on_console(&console, AssetKind::Cover, &mut cover_file(10));
    select_halo(&mut harness);
    let refused = Cell::new(false);
    harness.app().download(KindChoice::All);

    harness.run_jobs_with(|app, stage| {
        if !matches!(stage, OutcomeStage::Progress(_)) || refused.get() {
            return;
        }
        assert!(app.locks().editor);
        let stray = vec![PathBuf::from("not-loaded.asset")];
        app.open_paths(stray.clone());
        assert_eq!(app.status().text, BUSY_TEXT);
        app.files_dropped(stray);
        assert_eq!(app.status().text, BUSY_TEXT);
        refused.set(true);
    });

    assert!(refused.get(), "the download reported no progress");
    assert_eq!(harness.queued_jobs(), 0);
    assert_eq!(harness.app().modal(), None);
    assert!(!harness.app().locks().editor);
}
