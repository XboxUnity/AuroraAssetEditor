use std::cell::Cell;
use std::io;
use std::path::PathBuf;

use aurora_asset::{AssetKind, AssetSlot, Compression, RgbaImage};
use aurora_asset_editor::app::{
    BUSY_TEXT, CONNECTION_FAILED_TITLE, NO_CONSOLE_TEXT, REFRESHING_TEXT, TEMP_FILE_NOTE,
};
use aurora_asset_editor::console::{ConsoleLink, GameColumn, PresenceCell, port_text_for_save};
use aurora_asset_editor::dialogs::{ConnectionForm, Modal, ModalAnswer, ModalField};
use aurora_asset_editor::jobs::{ConsoleJobKind, ConsoleOutcome, JobOutcome, Progress};
use aurora_asset_editor::status::Severity;
use aurora_console::{
    AssetPresence, ConsoleError, ConsoleStatus, FTP_SETTINGS_FILE, FtpSettings, Game, GameIds,
    GameListing, HexId, TempCleanupError, asset_file_name, asset_folder, read_content_db,
};
use editor_test_support::{AppHarness, OutcomeStage, asset_file_with};
use test_servers::{FakeConsole, FakeConsoleConfig};

const LAST_REFRESH_TEXT: &str = "last refresh 20:41";

fn typical_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/console/content-db/typical.db")
}

fn typical_games() -> Vec<Game> {
    read_content_db(&typical_db_path()).expect("the sample database reads")
}

fn console_with_typical_games() -> FakeConsole {
    let typical = std::fs::read(typical_db_path()).expect("the sample database is readable");
    FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical)
}

fn harness_with_console() -> (AppHarness, FakeConsole) {
    let console = console_with_typical_games();
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    (harness, console)
}

fn refreshed(harness: &mut AppHarness) {
    harness.app().refresh_games();
    harness.run_jobs();
}

fn form(harness: &mut AppHarness) -> ConnectionForm {
    match harness.app().modal() {
        Some(Modal::Connection { form }) => form.clone(),
        other => panic!("expected the connection dialog, got {other:?}"),
    }
}

fn type_into_form(harness: &mut AppHarness, host: &str, port: &str) {
    let app = &mut *harness.app();
    app.edit_modal_field(ModalField::Host, host);
    app.edit_modal_field(ModalField::Port, port);
}

fn visible_titles(harness: &mut AppHarness) -> Vec<String> {
    harness
        .app()
        .console()
        .table
        .visible()
        .iter()
        .map(|game| game.title_name.clone())
        .collect()
}

fn game(title: &str, title_id: &str, database_id: i64, disc: u32) -> Game {
    Game {
        title_name: title.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(title_id).unwrap(),
            database_id: HexId::from_database_value(database_id),
        },
        media_id: HexId::parse("00000000").unwrap(),
        disc,
    }
}

/// Fills the table as a refresh would, without a console.
fn show_games(harness: &mut AppHarness, games: Vec<Game>) {
    let app = &mut *harness.app();
    app.refresh_games();
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Ok(GameListing {
            games,
            cleanup_error: None,
        }),
    }));
}

fn mixed_case_games() -> Vec<Game> {
    vec![
        game("banana", "4D5307E6", 1, 2),
        game("Apple", "4D5307E7", 2, 10),
        game("cherry", "4E5307E6", 3, 1),
        game("apple pie", "4D5307E8", 4, 1),
    ]
}

fn console_has_cover_for(console: &FakeConsole, ids: &GameIds) {
    let mut cover = asset_file_with(&[(
        AssetSlot::Cover,
        RgbaImage::from_pixel(8, 8, image::Rgba([10, 40, 90, 255])),
        Compression::None,
    )]);
    let path = format!(
        "{}/{}",
        asset_folder(ids),
        asset_file_name(AssetKind::Cover, &ids.title_id)
    );
    console.add_file(&path, &cover.to_bytes());
}

#[test]
fn connection_form_prefills_defaults_and_subnet() {
    let mut harness = AppHarness::new();

    harness.app().edit_connection();

    let form = form(&mut harness);
    assert_eq!(form.host, "192.168.1.");
    assert_eq!(form.port, "21");
    assert_eq!(form.username, "xboxftp");
    assert_eq!(form.password, "xboxftp");
    assert_eq!(form.test_status, None);
}

#[test]
fn connection_form_prefills_saved_settings() {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: "10.0.0.7".to_owned(),
        port: "2121".to_owned(),
        username: "me".to_owned(),
        password: "secret".to_owned(),
    });

    harness.app().edit_connection();

    let form = form(&mut harness);
    assert_eq!(form.host, "10.0.0.7");
    assert_eq!(form.port, "2121");
    assert_eq!(form.username, "me");
    assert_eq!(form.password, "secret");
}

#[test]
fn testing_connection_reports_revision() {
    let (mut harness, _console) = harness_with_console();
    harness.app().edit_connection();

    harness.app().test_connection();
    harness.run_jobs();

    let expected = "Connection to Aurora revision 1655 Established...";
    assert_eq!(
        form(&mut harness).test_status,
        Some((Severity::Info, expected.to_owned()))
    );
    assert_eq!(harness.app().status().text, expected);
    assert_eq!(harness.app().status().progress, None);
    assert_eq!(
        harness.app().console().link,
        ConsoleLink::Connected {
            revision: "1655".to_owned()
        }
    );
}

#[test]
fn failed_test_shows_error_and_marks_link_failed() {
    let (mut harness, console) = harness_with_console();
    console.refuse_new_connections();
    harness.app().edit_connection();

    harness.app().test_connection();
    harness.run_jobs();

    let (severity, text) = form(&mut harness).test_status.expect("the test ended");
    assert_eq!(severity, Severity::Error);
    assert!(text.starts_with("Error: could not connect to 127.0.0.1 on port"));
    assert_eq!(harness.app().status().severity, Severity::Error);
    assert_eq!(harness.app().console().link, ConsoleLink::Failed);
    assert!(
        harness
            .error_log_text()
            .contains("The connection test failed")
    );
}

#[test]
fn testing_connection_applies_settings_without_saving() {
    let console = console_with_typical_games();
    let mut harness = AppHarness::new();
    harness.app().edit_connection();
    type_into_form(&mut harness, "127.0.0.1", &console.port().to_string());

    harness.app().test_connection();
    harness.run_jobs();

    assert!(!harness.config_dir().join(FTP_SETTINGS_FILE).exists());
    harness.app().answer_modal(ModalAnswer::Cancel);
    refreshed(&mut harness);
    assert_eq!(harness.app().console().table.visible().len(), 3);
}

#[test]
fn saving_connection_writes_ftp_json() {
    let mut harness = AppHarness::new();
    harness.app().edit_connection();
    type_into_form(&mut harness, "10.0.0.7", "2121");
    harness.app().edit_modal_field(ModalField::Username, "me");
    harness
        .app()
        .edit_modal_field(ModalField::Password, "secret");

    harness.app().answer_modal(ModalAnswer::Accept);

    let path = harness.config_dir().join(FTP_SETTINGS_FILE);
    let saved = FtpSettings::load(&path).unwrap().expect("ftp.json exists");
    assert_eq!(saved.host, "10.0.0.7");
    assert_eq!(saved.port, "2121");
    assert_eq!(saved.username, "me");
    assert_eq!(saved.password, "secret");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"ip": "10.0.0.7", "pass": "secret", "port": "2121", "user": "me"})
    );
    assert_eq!(harness.app().modal(), None);
    assert_eq!(harness.app().console().settings, Some(saved));
}

#[test]
fn unparsable_port_is_saved_as_21() {
    let cases = [("abc", "21"), ("0", "0"), (" 2121 ", " 2121 ")];
    for (typed, saved) in cases {
        let mut harness = AppHarness::new();
        harness.app().edit_connection();
        type_into_form(&mut harness, "10.0.0.7", typed);

        harness.app().save_connection();

        let path = harness.config_dir().join(FTP_SETTINGS_FILE);
        let loaded = FtpSettings::load(&path).unwrap().expect("ftp.json exists");
        assert_eq!(loaded.port, saved, "typed {typed:?}");
        assert_eq!(port_text_for_save(typed), saved);
    }
}

#[test]
fn failed_save_keeps_the_form_and_says_why() {
    let mut harness = AppHarness::new();
    std::fs::create_dir(harness.config_dir().join(FTP_SETTINGS_FILE)).unwrap();
    harness.app().edit_connection();

    harness.app().save_connection();

    let (severity, text) = form(&mut harness)
        .test_status
        .expect("the failure is shown");
    assert_eq!(severity, Severity::Error);
    assert!(text.starts_with("Could not save the connection settings: could not write"));
    assert!(
        harness
            .error_log_text()
            .contains("Could not save the connection settings")
    );
}

#[test]
fn refresh_without_settings_shows_message() {
    let mut harness = AppHarness::new();

    harness.app().refresh_games();

    assert_eq!(harness.app().status().text, NO_CONSOLE_TEXT);
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn refresh_clears_list_immediately() {
    let (mut harness, _console) = harness_with_console();
    refreshed(&mut harness);
    assert_eq!(harness.app().console().table.visible().len(), 3);

    harness.app().refresh_games();

    assert!(harness.app().console().table.visible().is_empty());
    assert_eq!(harness.app().status().text, REFRESHING_TEXT);
    assert_eq!(harness.queued_jobs(), 1);
}

#[test]
fn refresh_lists_games() {
    let (mut harness, _console) = harness_with_console();

    refreshed(&mut harness);

    let rows: Vec<Game> = harness
        .app()
        .console()
        .table
        .visible()
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(rows, typical_games());
    assert_eq!(
        harness.app().status().text,
        format!("Connected · 3 games · {LAST_REFRESH_TEXT}")
    );
    assert!(harness.app().console().last_refresh.is_some());
}

#[test]
fn refresh_connection_failure_shows_dialog() {
    let (mut harness, console) = harness_with_console();
    console.refuse_new_connections();

    harness.app().refresh_games();
    harness.run_jobs();

    let Some(Modal::Message { title, text }) = harness.app().modal().cloned() else {
        panic!("expected a message dialog");
    };
    assert_eq!(title, CONNECTION_FAILED_TITLE);
    assert!(text.starts_with("FTP Connection could not be established: could not connect to"));
    assert_eq!(harness.app().console().link, ConsoleLink::Failed);
    assert!(
        harness
            .error_log_text()
            .contains("could not connect to 127.0.0.1")
    );
}

#[test]
fn refresh_failure_after_connecting_goes_to_status_line() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut harness = AppHarness::new();
    harness.with_console(&console);

    refreshed(&mut harness);

    assert_eq!(harness.app().modal(), None);
    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    assert!(status.text.starts_with("Could not refresh the game list: "));
    assert!(
        harness
            .error_log_text()
            .contains("Could not refresh the game list")
    );
}

#[test]
fn temp_cleanup_failure_is_logged() {
    let (mut harness, _console) = harness_with_console();
    harness.app().refresh_games();
    let generation = harness.app().console().refresh_generation();
    let listing = GameListing {
        games: typical_games(),
        cleanup_error: Some(TempCleanupError {
            path: PathBuf::from("AuroraAssetEditor-1.db"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        }),
    };

    harness
        .app()
        .on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
            generation,
            result: Ok(listing),
        }));

    assert!(
        harness
            .app()
            .status()
            .text
            .ends_with(&format!(" · {TEMP_FILE_NOTE}"))
    );
    let log = harness.error_log_text();
    assert!(log.contains("could not remove the temporary copy AuroraAssetEditor-1.db"));
    assert_eq!(harness.app().console().table.visible().len(), 3);
}

#[test]
fn game_rows_show_x08_ids_and_disc() {
    let (mut harness, _console) = harness_with_console();
    refreshed(&mut harness);

    let rows: Vec<(String, String, String, String, u32)> = harness
        .app()
        .console()
        .table
        .visible()
        .iter()
        .map(|game| {
            (
                game.title_name.clone(),
                game.ids.title_id.to_string(),
                game.ids.database_id.to_string(),
                game.media_id.to_string(),
                game.disc,
            )
        })
        .collect();

    let row = |title: &str, title_id: &str, database_id: &str, media_id: &str, disc| {
        (
            title.to_owned(),
            title_id.to_owned(),
            database_id.to_owned(),
            media_id.to_owned(),
            disc,
        )
    };
    assert_eq!(
        rows[0],
        row("Halo 3", "4D5307E6", "00000001", "1A2B3C4D", 1)
    );
    assert_eq!(rows[1].2, "00000002");
    assert_eq!(
        rows[2],
        row("Hexic HD", "58410A1C", "00000012", "00000000", 1)
    );
}

#[test]
fn filters_are_anded() {
    let mut harness = AppHarness::new();
    show_games(&mut harness, mixed_case_games());

    harness.app().set_game_filter("APPLE", "");
    assert_eq!(visible_titles(&mut harness), ["Apple", "apple pie"]);

    harness.app().set_game_filter("apple", "4d5307e7");
    assert_eq!(visible_titles(&mut harness), ["Apple"]);

    harness.app().set_game_filter("", "4e");
    assert_eq!(visible_titles(&mut harness), ["cherry"]);

    harness.app().set_game_filter("banana", "4e");
    assert!(visible_titles(&mut harness).is_empty());
}

#[test]
fn sorting_toggles_direction() {
    let mut harness = AppHarness::new();
    show_games(&mut harness, mixed_case_games());
    assert_eq!(harness.app().console().table.sort(), None);
    assert_eq!(
        visible_titles(&mut harness),
        ["banana", "Apple", "cherry", "apple pie"]
    );

    harness.app().sort_games(GameColumn::Disc);
    assert_eq!(
        harness.app().console().table.sort(),
        Some((GameColumn::Disc, true))
    );
    assert_eq!(
        visible_titles(&mut harness),
        ["cherry", "apple pie", "banana", "Apple"]
    );

    harness.app().sort_games(GameColumn::Disc);
    assert_eq!(
        harness.app().console().table.sort(),
        Some((GameColumn::Disc, false))
    );
    assert_eq!(
        visible_titles(&mut harness),
        ["Apple", "banana", "cherry", "apple pie"]
    );

    harness.app().sort_games(GameColumn::TitleId);
    assert_eq!(
        harness.app().console().table.sort(),
        Some((GameColumn::TitleId, true))
    );
    assert_eq!(
        visible_titles(&mut harness),
        ["banana", "Apple", "apple pie", "cherry"]
    );
}

#[test]
fn title_sort_ignores_case() {
    let mut harness = AppHarness::new();
    show_games(&mut harness, mixed_case_games());

    harness.app().sort_games(GameColumn::Title);

    assert_eq!(
        visible_titles(&mut harness),
        ["Apple", "apple pie", "banana", "cherry"]
    );
}

#[test]
fn selecting_a_game_sets_current_game_and_online_title_id() {
    let mut harness = AppHarness::new();
    show_games(&mut harness, mixed_case_games());
    harness.app().sort_games(GameColumn::Title);

    harness.app().select_game(2);

    let current = harness
        .app()
        .current_game()
        .cloned()
        .expect("a game is selected");
    assert_eq!(current.title, "banana");
    assert_eq!(current.ids.title_id.as_str(), "4D5307E6");
    assert_eq!(current.ids.database_id.as_str(), "00000001");
    let selected = harness.app().console().table.selected().cloned();
    assert_eq!(
        selected.map(|game| game.title_name),
        Some("banana".to_owned())
    );
    assert_eq!(harness.app().online().title_id_text, "4D5307E6");
}

#[test]
fn clearing_game_clears_table_selection() {
    let mut harness = AppHarness::new();
    show_games(&mut harness, mixed_case_games());
    harness.app().select_game(1);
    assert!(harness.app().console().table.selected().is_some());

    harness.app().clear_current_game();

    assert!(harness.app().current_game().is_none());
    assert!(harness.app().console().table.selected().is_none());
    assert_eq!(harness.app().online().title_id_text, "");
}

#[test]
fn presence_fills_lazily_after_refresh() {
    let (mut harness, console) = harness_with_console();
    let games = typical_games();
    console_has_cover_for(&console, &games[0].ids);
    harness.app().refresh_games();
    let mut after_refresh = Vec::new();

    harness.run_jobs_with(|app, stage| {
        if stage == OutcomeStage::Final && after_refresh.is_empty() {
            after_refresh = games
                .iter()
                .map(|game| app.console().table.presence(&game.ids))
                .collect();
        }
    });

    assert_eq!(after_refresh, [PresenceCell::Unknown; 3]);
    let with_cover = AssetPresence {
        cover: true,
        ..AssetPresence::NONE
    };
    let presence: Vec<PresenceCell> = typical_games()
        .iter()
        .map(|game| harness.app().console().table.presence(&game.ids))
        .collect();
    assert_eq!(
        presence,
        [
            PresenceCell::Known(with_cover),
            PresenceCell::Known(AssetPresence::NONE),
            PresenceCell::Known(AssetPresence::NONE),
        ]
    );
}

#[test]
fn presence_follows_visible_order_then_hidden_games() {
    let (mut harness, _console) = harness_with_console();
    refreshed(&mut harness);
    harness.app().sort_games(GameColumn::Title);
    harness.app().set_game_filter("hexic", "");

    let order = harness.app().console().table.ids_in_presence_order();

    let titles: Vec<String> = order.iter().map(|ids| ids.title_id.to_string()).collect();
    assert_eq!(titles, ["58410A1C", "4D5307E6", "4D530842"]);
}

#[test]
fn presence_from_previous_refresh_is_ignored() {
    let (mut harness, _console) = harness_with_console();
    refreshed(&mut harness);
    let games = typical_games();
    harness.app().refresh_games();
    let stale_generation = harness.app().console().refresh_generation() - 1;
    let stale_everything = AssetPresence {
        cover: true,
        background: true,
        icon_banner: true,
        screenshots: true,
    };
    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Presence {
        generation: stale_generation,
        ids: games[1].ids.clone(),
        result: Ok(stale_everything),
    }));
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Presence {
        generation: stale_generation,
        ids: games[2].ids.clone(),
        result: Err(ConsoleError::LoginRejected { code: 530 }),
    }));

    assert_eq!(
        app.console().table.presence(&games[1].ids),
        PresenceCell::Unknown
    );
    assert_eq!(
        app.console().table.presence(&games[2].ids),
        PresenceCell::Unknown
    );
    drop(app_guard);
    assert!(harness.error_log_text().contains("reply 530"));
    harness.run_jobs();
    assert_eq!(
        harness.app().console().table.presence(&games[1].ids),
        PresenceCell::Known(AssetPresence::NONE)
    );
}

#[test]
fn failed_presence_check_marks_the_cell_and_is_logged() {
    let (mut harness, _console) = harness_with_console();
    refreshed(&mut harness);
    let ids = typical_games()[0].ids.clone();
    let generation = harness.app().console().refresh_generation();

    harness
        .app()
        .on_job_outcome(JobOutcome::Console(ConsoleOutcome::Presence {
            generation,
            ids: ids.clone(),
            result: Err(ConsoleError::TooManyConnections),
        }));

    assert_eq!(
        harness.app().console().table.presence(&ids),
        PresenceCell::Failed
    );
    assert!(harness.error_log_text().contains("no more FTP connections"));
}

#[test]
fn user_jobs_run_before_presence_jobs() {
    let (mut harness, _console) = harness_with_console();
    let games = typical_games();
    harness.app().refresh_games();
    let finals = Cell::new(0);
    let presence_when_user_job_ended = Cell::new(None);

    harness.run_jobs_with(|app, stage| {
        if stage != OutcomeStage::Final {
            return;
        }
        finals.set(finals.get() + 1);
        if finals.get() == 1 {
            app.open_paths(vec![PathBuf::from("no-such-file.asset")]);
        } else if finals.get() == 2 {
            presence_when_user_job_ended.set(Some(
                games
                    .iter()
                    .map(|game| app.console().table.presence(&game.ids))
                    .collect::<Vec<_>>(),
            ));
        }
    });

    assert_eq!(
        presence_when_user_job_ended.take(),
        Some(vec![PresenceCell::Unknown; 3])
    );
    assert_eq!(finals.get(), 5);
}

#[test]
fn console_status_reaches_status_line() {
    let (mut harness, _console) = harness_with_console();
    harness.app().refresh_games();
    let mut first_progress = None;

    harness.run_jobs_with(|app, stage| {
        if first_progress.is_none()
            && let OutcomeStage::Progress(progress) = stage
        {
            first_progress = Some((progress, app.status().progress.clone()));
        }
    });

    let connecting = ConsoleStatus::Connecting {
        host: "127.0.0.1".to_owned(),
    };
    assert_eq!(
        first_progress,
        Some((
            Progress::Console {
                kind: ConsoleJobKind::Refresh,
                status: connecting
            },
            Some("Connecting to 127.0.0.1...".to_owned())
        ))
    );
}

#[test]
fn test_connection_shows_each_status_in_the_form() {
    let (mut harness, _console) = harness_with_console();
    harness.app().edit_connection();
    harness.app().test_connection();
    let mut lines = Vec::new();

    harness.run_jobs_with(|app, stage| {
        if matches!(stage, OutcomeStage::Progress(_))
            && let Some(Modal::Connection { form }) = app.modal()
        {
            lines.push(form.test_status.clone());
        }
    });

    let info = |text: &str| Some((Severity::Info, text.to_owned()));
    assert_eq!(
        lines,
        [
            info("Connecting to 127.0.0.1..."),
            info("Connected to 127.0.0.1, checking for Aurora..."),
            info("Connected to Aurora revision 1655"),
        ]
    );
}

#[test]
fn stale_presence_jobs_do_not_contact_the_console() {
    let (mut harness, console) = harness_with_console();
    harness.app().refresh_games();
    harness.run_jobs_with(|app, stage| {
        if stage == OutcomeStage::Final && app.console().refresh_generation() == 1 {
            app.refresh_games();
        }
    });

    let checks_of_first_game = console
        .commands()
        .iter()
        .filter(|command| command.contains("GameData/4D5307E6_00000001"))
        .count();
    assert_eq!(harness.app().console().refresh_generation(), 2);
    assert_eq!(checks_of_first_game, 1, "{:?}", console.commands());
    let presence: Vec<PresenceCell> = typical_games()
        .iter()
        .map(|game| harness.app().console().table.presence(&game.ids))
        .collect();
    assert_eq!(presence, [PresenceCell::Known(AssetPresence::NONE); 3]);
}

#[test]
fn unreachable_console_ends_the_presence_checks() {
    let (mut harness, console) = harness_with_console();
    harness.app().refresh_games();

    let mut console_gone = false;
    harness.run_jobs_with(|_, stage| {
        if stage == OutcomeStage::Final && !console_gone {
            console_gone = true;
            console.drop_connections();
            console.refuse_new_connections();
        }
    });

    assert_eq!(harness.app().console().link, ConsoleLink::Failed);
    let presence: Vec<PresenceCell> = typical_games()
        .iter()
        .map(|game| harness.app().console().table.presence(&game.ids))
        .collect();
    assert_eq!(presence, [PresenceCell::Failed; 3]);
    let log = harness.error_log_text();
    assert_eq!(
        log.matches("The console could not be reached to check the assets")
            .count(),
        1,
        "{log}"
    );
    assert_eq!(harness.queued_jobs(), 0);
}

#[test]
fn presence_progress_does_not_reach_the_status_line() {
    let (mut harness, _console) = harness_with_console();
    harness.app().refresh_games();
    let mut leaked = Vec::new();

    harness.run_jobs_with(|app, stage| {
        let presence_progress = matches!(
            stage,
            OutcomeStage::Progress(Progress::Console {
                kind: ConsoleJobKind::Presence,
                ..
            })
        );
        if presence_progress && app.status().progress.is_some() {
            leaked.push(app.status().progress.clone());
        }
    });

    assert!(leaked.is_empty(), "{leaked:?}");
}

#[test]
fn console_that_is_not_aurora_keeps_the_old_test_text() {
    let config = FakeConsoleConfig {
        site_revision_reply: None,
        ..FakeConsoleConfig::default()
    };
    let console = FakeConsole::start(config);
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    harness.app().edit_connection();

    harness.app().test_connection();
    harness.run_jobs();

    let (severity, text) = form(&mut harness).test_status.expect("the test ended");
    assert_eq!(severity, Severity::Error);
    assert!(
        text.starts_with(
            "Connection test Failed...: the console does not look like it runs Aurora"
        ),
        "{text}"
    );
}

#[test]
fn test_refused_while_busy_says_so_in_the_form() {
    let (mut harness, _console) = harness_with_console();
    harness.app().edit_connection();
    harness.app().test_connection();

    harness.app().test_connection();

    assert_eq!(
        form(&mut harness).test_status,
        Some((Severity::Info, BUSY_TEXT.to_owned()))
    );
    assert_eq!(harness.queued_jobs(), 1);
}

#[test]
fn test_connection_applies_the_port_rewrite_to_the_session() {
    let mut harness = AppHarness::new();
    harness.app().edit_connection();
    type_into_form(&mut harness, "10.0.0.7", "abc");

    harness.app().test_connection();

    let settings = harness.app().console().settings.clone().expect("applied");
    assert_eq!(settings.host, "10.0.0.7");
    assert_eq!(settings.port, "21");
}

#[test]
fn presence_checks_run_for_a_hand_built_generation_zero_refresh() {
    let (mut harness, _console) = harness_with_console();
    let games = typical_games();
    let first = games[0].ids.clone();
    let mut app_guard = harness.app();
    let app = &mut *app_guard;
    let generation = app.console().refresh_generation();
    assert_eq!(generation, 0);
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Ok(GameListing {
            games,
            cleanup_error: None,
        }),
    }));

    drop(app_guard);
    harness.run_jobs();

    assert!(matches!(
        harness.app().console().table.presence(&first),
        PresenceCell::Known(_)
    ));
}
