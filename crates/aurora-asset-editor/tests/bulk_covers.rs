use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use aurora_asset::{AssetFile, AssetSlot, Compression, RgbaImage, decode, encode};
use aurora_asset_editor::app::{
    BULK_FINISHED_TITLE, BULK_TITLE, CONNECTION_FAILED_TITLE, NO_ASSETS_TEXT, NO_CONSOLE_TEXT,
};
use aurora_asset_editor::console::{
    ConsoleLink, CoverProviderError, GameColumn, OnlineCoverProvider, PresenceCell,
};
use aurora_asset_editor::dialogs::{BulkForm, Modal, ModalAnswer};
use aurora_asset_editor::jobs::{ConsoleOutcome, JobOutcome, Progress};
use aurora_console::{
    CoverProvider, Game, GameListing, asset_file_name, asset_folder, read_content_db,
};
use aurora_image::{decode_image, encoder_pixels};
use aurora_online::{CoverSource, Endpoints, OnlineClient};
use editor_test_support::{AppHarness, OutcomeStage};
use test_servers::{FakeConsole, FakeConsoleConfig, RecordedHttp, Route, RouteBehaviour};

const COVER_WIDTH: u32 = 100;
const COVER_HEIGHT: u32 = 50;
const ONLINE_TIMEOUT: Duration = Duration::from_secs(5);

fn typical_db() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/console/content-db/typical.db");
    std::fs::read(path).expect("the sample database is readable")
}

fn typical_game(title: &str) -> Game {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/console/content-db/typical.db");
    read_content_db(&path)
        .expect("the sample database reads")
        .into_iter()
        .find(|game| game.title_name == title)
        .expect("the sample database lists the game")
}

fn echo() -> Game {
    typical_game("Fable II")
}

fn foxtrot() -> Game {
    typical_game("Halo 3")
}

fn delta() -> Game {
    typical_game("Hexic HD")
}

/// Leaves Fable II and Halo 3 in the table, in title order.
fn show_two_games(harness: &mut AppHarness) {
    let app = &mut *harness.app();
    app.set_game_filter("l", "");
    app.sort_games(GameColumn::Title);
}

fn png(width: u32, height: u32, shade: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([shade, (x % 251) as u8, (y % 251) as u8, 255])
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("a PNG encodes into memory");
    bytes
}

fn route(path: &str, status: u16, content_type: &str, body: Vec<u8>) -> Route {
    Route {
        path: path.to_owned(),
        status,
        content_type: content_type.to_owned(),
        body,
        behaviour: RouteBehaviour::Normal,
    }
}

/// The XboxUnity answers for `games`: one cover each, a PNG of `cover_size`. A title in `failing` answers 500.
fn cover_routes(games: &[Game], cover_size: (u32, u32), failing: &[&str]) -> Vec<Route> {
    let mut routes = Vec::new();
    for game in games {
        let title_id = game.ids.title_id.as_str().to_owned();
        if failing.contains(&title_id.as_str()) {
            routes.push(route(
                &format!("/api/Covers/{title_id}"),
                500,
                "text/plain",
                b"broken".to_vec(),
            ));
            continue;
        }
        let listing = format!(
            r#"[{{"name":"{}","official":true,"rating":"5","url":"{{server}}/image/{title_id}.png","thumbnail":null,"titleid":"{title_id}"}}]"#,
            game.title_name
        );
        routes.push(route(
            &format!("/api/Covers/{title_id}"),
            200,
            "application/json",
            listing.into_bytes(),
        ));
        routes.push(route(
            &format!("/image/{title_id}.png"),
            200,
            "image/png",
            png(cover_size.0, cover_size.1, 40),
        ));
    }
    routes
}

fn endpoints(server: &RecordedHttp) -> Endpoints {
    Endpoints {
        xboxunity_covers: format!("{}/api/Covers/", server.base_url()),
        archive_covers: format!("{}/archive/", server.base_url()),
    }
}

struct Fixture {
    harness: AppHarness,
    console: FakeConsole,
    _server: RecordedHttp,
}

/// An app that refreshed against the sample database's console, which has the games' folders, with the
/// online source answering `routes`.
fn fixture_with_routes(routes: Vec<Route>) -> Fixture {
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical_db());
    for game in [delta(), echo(), foxtrot()] {
        console.add_dir(&asset_folder(&game.ids));
    }
    let server = RecordedHttp::start(routes);
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    harness.with_online(endpoints(&server));
    harness.app().refresh_games();
    harness.run_jobs();
    Fixture {
        harness,
        console,
        _server: server,
    }
}

fn fixture() -> Fixture {
    fixture_with_routes(all_cover_routes(&[]))
}

fn all_cover_routes(failing: &[&str]) -> Vec<Route> {
    cover_routes(
        &[delta(), echo(), foxtrot()],
        (COVER_WIDTH, COVER_HEIGHT),
        failing,
    )
}

fn start_bulk(harness: &mut AppHarness) {
    let app = &mut *harness.app();
    app.open_bulk_actions();
    app.answer_modal(ModalAnswer::Accept);
}

fn cover_name(game: &Game) -> String {
    asset_file_name(aurora_asset::AssetKind::Cover, &game.ids.title_id)
}

fn uploaded_cover(console: &FakeConsole, game: &Game) -> Option<AssetFile> {
    let path = format!("{}/{}", asset_folder(&game.ids), cover_name(game));
    console
        .file(&path)
        .map(|bytes| AssetFile::parse(&bytes).expect("an uploaded asset parses"))
}

/// The cover files stored since `already_seen` commands, in order.
fn stored_covers(console: &FakeConsole, already_seen: usize) -> Vec<String> {
    console
        .commands()
        .into_iter()
        .skip(already_seen)
        .filter_map(|command| command.strip_prefix("STOR ").map(str::to_owned))
        .collect()
}

fn message(harness: &mut AppHarness, expected_title: &str) -> String {
    match harness.app().modal() {
        Some(Modal::Message { title, text }) => {
            assert_eq!(title, expected_title);
            text.clone()
        }
        other => panic!("expected the message {expected_title:?}, got {other:?}"),
    }
}

fn expected_texture(shade: u8, compression: Compression) -> aurora_asset::EncodedTexture {
    let decoded = decode_image(&png(COVER_WIDTH, COVER_HEIGHT, shade), None).unwrap();
    encode(&encoder_pixels(&decoded), compression).unwrap()
}

#[test]
fn bulk_with_empty_list_shows_old_message() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut harness = AppHarness::new();
    harness.with_console(&console);

    harness.app().open_bulk_actions();

    assert_eq!(message(&mut harness, BULK_TITLE), NO_ASSETS_TEXT);
    assert_eq!(NO_ASSETS_TEXT, "No Assets listed");
    assert_eq!(BULK_TITLE, "Bulk actions");
}

#[test]
fn bulk_dialog_defaults() {
    let mut fixture = fixture();
    let app = &mut *fixture.harness.app();

    app.open_bulk_actions();

    assert_eq!(
        app.modal(),
        Some(&Modal::Bulk {
            form: BulkForm {
                source: CoverSource::XboxUnity,
                replace_existing: false,
                game_count: 3,
            }
        })
    );
    app.set_bulk_source(CoverSource::InternetArchive);
    app.set_bulk_replace(true);
    assert_eq!(
        app.modal(),
        Some(&Modal::Bulk {
            form: BulkForm {
                source: CoverSource::InternetArchive,
                replace_existing: true,
                game_count: 3,
            }
        })
    );
}

#[test]
fn bulk_runs_on_filtered_list_in_view_order() {
    let mut fixture = fixture();
    show_two_games(&mut fixture.harness);
    let already_seen = fixture.console.commands().len();

    start_bulk(&mut fixture.harness);
    fixture.harness.run_jobs();

    assert_eq!(
        stored_covers(&fixture.console, already_seen),
        vec![cover_name(&echo()), cover_name(&foxtrot()),]
    );
    assert!(uploaded_cover(&fixture.console, &delta()).is_none());
    let app = &mut *fixture.harness.app();
    for uploaded in [echo(), foxtrot()] {
        match app.console().table.presence(&uploaded.ids) {
            PresenceCell::Known(presence) => assert!(presence.cover),
            other => panic!("expected a known presence, got {other:?}"),
        }
    }
    match app.console().table.presence(&delta().ids) {
        PresenceCell::Known(presence) => assert!(!presence.cover),
        other => panic!("expected a known presence, got {other:?}"),
    }
}

#[test]
fn bulk_cover_is_not_resized() {
    let mut fixture = fixture();
    fixture
        .harness
        .app()
        .set_game_filter("", echo().ids.title_id.as_str());

    start_bulk(&mut fixture.harness);
    fixture.harness.run_jobs();

    let file = uploaded_cover(&fixture.console, &echo()).expect("the cover was uploaded");
    let texture = file.get(AssetSlot::Cover).expect("the file holds a cover");
    let decoded = decode(texture).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (COVER_WIDTH, COVER_HEIGHT)
    );
    assert_eq!(texture, &expected_texture(40, Compression::Dxt));
}

#[test]
fn bulk_uses_compression_setting() {
    for (use_compression, compression) in [(true, Compression::Dxt), (false, Compression::None)] {
        let mut fixture = fixture();
        fixture
            .harness
            .app()
            .set_game_filter("", echo().ids.title_id.as_str());
        fixture.harness.app().set_use_compression(use_compression);

        start_bulk(&mut fixture.harness);
        fixture.harness.run_jobs();

        let file = uploaded_cover(&fixture.console, &echo()).expect("the cover was uploaded");
        assert_eq!(
            file.get(AssetSlot::Cover),
            Some(&expected_texture(40, compression))
        );
    }
    assert_ne!(
        expected_texture(40, Compression::Dxt),
        expected_texture(40, Compression::None)
    );
}

#[test]
fn bulk_stop_halts_before_next_game() {
    let mut fixture = fixture();
    show_two_games(&mut fixture.harness);
    start_bulk(&mut fixture.harness);

    fixture.harness.run_jobs_with(|app, stage| {
        if matches!(stage, OutcomeStage::Progress(Progress::Bulk { .. })) {
            app.stop_bulk();
        }
    });

    assert!(uploaded_cover(&fixture.console, &echo()).is_some());
    assert!(uploaded_cover(&fixture.console, &foxtrot()).is_none());
    let text = message(&mut fixture.harness, BULK_FINISHED_TITLE);
    assert!(text.contains("Saved to the console: 1"), "{text}");
    assert!(text.contains("Not handled: 1"), "{text}");
    assert!(text.contains("Stopped: the run was stopped"), "{text}");
    assert!(!fixture.harness.app().locks().bulk_stop);
}

#[test]
fn bulk_console_gone_is_logged() {
    let mut fixture = fixture();
    fixture.harness.app().sort_games(GameColumn::Title);
    let console = &fixture.console;
    start_bulk(&mut fixture.harness);

    let mut handled = 0;
    fixture.harness.run_jobs_with(|_, stage| {
        if let OutcomeStage::Progress(Progress::Bulk { .. }) = stage {
            handled += 1;
            if handled == 2 {
                console.drop_connections();
                console.refuse_new_connections();
            }
        }
    });

    let text = message(&mut fixture.harness, BULK_FINISHED_TITLE);
    assert!(text.contains("Saved to the console: 1"), "{text}");
    assert!(text.contains("Not handled: 2"), "{text}");
    assert!(
        text.contains("Stopped: the console could not be reached again"),
        "{text}"
    );
    let log = fixture.harness.error_log_text();
    assert!(
        log.contains("Bulk covers stopped, the console cannot be reached again"),
        "{log}"
    );
    assert!(
        log.contains("the console could not be reached again"),
        "{log}"
    );
    let cause = format!(
        "
  caused by: could not connect to 127.0.0.1 on port {}",
        fixture.console.port()
    );
    assert!(log.contains(&cause), "{log}");
    assert_eq!(fixture.harness.app().console().link, ConsoleLink::Failed);
}

#[test]
fn bulk_errors_continue_and_are_summarised_and_logged() {
    let failing = foxtrot().ids.title_id.as_str().to_owned();
    let mut fixture = fixture_with_routes(all_cover_routes(&[&failing]));
    fixture.harness.app().sort_games(GameColumn::Title);

    start_bulk(&mut fixture.harness);
    fixture.harness.run_jobs();

    assert!(uploaded_cover(&fixture.console, &delta()).is_some());
    assert!(uploaded_cover(&fixture.console, &echo()).is_some());
    assert!(uploaded_cover(&fixture.console, &foxtrot()).is_none());
    let text = message(&mut fixture.harness, BULK_FINISHED_TITLE);
    assert!(text.contains("Saved to the console: 2"), "{text}");
    assert!(text.contains("Failed: 1"), "{text}");
    assert_eq!(
        fixture.harness.app().status().text,
        "Bulk covers: 2 saved, 1 failed"
    );
    assert!(fixture.harness.app().toasts().iter().count() == 0);
    assert!(!text.contains("Stopped"), "{text}");
    let log = fixture.harness.error_log_text();
    assert!(log.contains("Halo 3 (4D5307E6)"), "{log}");
    assert!(!log.contains("Fable II"), "{log}");
    assert!(!log.contains("Hexic HD"), "{log}");
}

#[test]
fn bulk_progress_shows_in_status() {
    let mut fixture = fixture();
    show_two_games(&mut fixture.harness);
    start_bulk(&mut fixture.harness);

    let mut shown = Vec::new();
    fixture.harness.run_jobs_with(|app, stage| {
        if matches!(stage, OutcomeStage::Progress(Progress::Bulk { .. })) {
            shown.push(app.status().progress.clone());
        }
    });

    assert_eq!(
        shown,
        vec![
            Some("Bulk covers: 1 of 2 · Fable II".to_owned()),
            Some("Bulk covers: 2 of 2 · Halo 3".to_owned()),
        ]
    );
    assert_eq!(fixture.harness.app().status().progress, None);
}

#[test]
fn bulk_stop_is_the_only_console_control_while_running() {
    let mut fixture = fixture();
    fixture
        .harness
        .app()
        .set_game_filter("", delta().ids.title_id.as_str());
    assert!(!fixture.harness.app().locks().bulk_stop);
    start_bulk(&mut fixture.harness);

    let mut during = None;
    fixture.harness.run_jobs_with(|app, stage| {
        if matches!(stage, OutcomeStage::Progress(Progress::Bulk { .. })) {
            during = Some(app.locks());
        }
    });

    let during = during.expect("the run reported progress");
    assert!(during.bulk_stop);
    assert!(during.console_actions);
    assert!(during.console_editor_writes);
    let after = fixture.harness.app().locks();
    assert!(!after.bulk_stop);
    assert!(!after.console_actions);
}

#[test]
fn cover_provider_decodes_without_resizing() {
    let games = [echo()];
    let server = RecordedHttp::start(cover_routes(&games, (COVER_WIDTH, COVER_HEIGHT), &[]));
    let client = OnlineClient::without_proxy(endpoints(&server), ONLINE_TIMEOUT);
    let mut provider =
        OnlineCoverProvider::new(client, CoverSource::XboxUnity, fastrand::Rng::with_seed(1));

    let cover = provider
        .random_cover(&echo().ids.title_id)
        .expect("the cover is fetched")
        .expect("the title has a cover");

    let decoded: RgbaImage = decode_image(&png(COVER_WIDTH, COVER_HEIGHT, 40), None).unwrap();
    assert_eq!((cover.width(), cover.height()), (COVER_WIDTH, COVER_HEIGHT));
    assert_eq!(cover, encoder_pixels(&decoded));
}

#[test]
fn cover_provider_reports_a_title_without_cover_and_a_bad_answer() {
    let server = RecordedHttp::start(vec![
        route(
            &format!("/api/Covers/{}", echo().ids.title_id.as_str()),
            200,
            "application/json",
            b"[]".to_vec(),
        ),
        route(
            &format!("/api/Covers/{}", foxtrot().ids.title_id.as_str()),
            500,
            "text/plain",
            b"broken".to_vec(),
        ),
    ]);
    let client = OnlineClient::without_proxy(endpoints(&server), ONLINE_TIMEOUT);
    let mut provider =
        OnlineCoverProvider::new(client, CoverSource::XboxUnity, fastrand::Rng::with_seed(1));

    assert!(
        provider
            .random_cover(&echo().ids.title_id)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        provider.random_cover(&foxtrot().ids.title_id),
        Err(CoverProviderError::Online(_))
    ));
}

#[test]
fn bulk_when_console_refuses_connections_shows_connection_failed() {
    let mut fixture = fixture();
    fixture.harness.app().open_bulk_actions();
    fixture.console.drop_connections();
    fixture.console.refuse_new_connections();

    fixture.harness.app().answer_modal(ModalAnswer::Accept);
    fixture.harness.run_jobs();

    let text = message(&mut fixture.harness, CONNECTION_FAILED_TITLE);
    assert!(
        text.starts_with("FTP Connection could not be established"),
        "{text}"
    );
    assert_eq!(fixture.harness.app().console().link, ConsoleLink::Failed);
    let log = fixture.harness.error_log_text();
    assert!(log.contains("could not connect to 127.0.0.1"), "{log}");
}

#[test]
fn bulk_without_console_settings_says_so() {
    let mut harness = AppHarness::new();
    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Ok(GameListing {
            games: vec![echo()],
            cleanup_error: None,
        }),
    }));

    drop(app_guard);
    harness.app().open_bulk_actions();

    let app = &mut *harness.app();
    assert!(app.modal().is_none());
    assert_eq!(app.status().text, NO_CONSOLE_TEXT);
}

#[test]
fn bulk_logs_an_unreadable_existing_cover_and_leaves_it() {
    let mut fixture = fixture();
    let garbage = b"this is not an asset file".to_vec();
    let path = format!("{}/{}", asset_folder(&echo().ids), cover_name(&echo()));
    fixture.console.add_file(&path, &garbage);
    fixture
        .harness
        .app()
        .set_game_filter("", echo().ids.title_id.as_str());

    start_bulk(&mut fixture.harness);
    fixture.harness.run_jobs();

    assert_eq!(fixture.console.file(&path), Some(garbage));
    let text = message(&mut fixture.harness, BULK_FINISHED_TITLE);
    assert!(
        text.contains("Cover on the console could not be read: 1"),
        "{text}"
    );
    let log = fixture.harness.error_log_text();
    let expected = format!("of Fable II ({})", echo().ids.title_id);
    assert!(log.contains(&expected), "{log}");
}
