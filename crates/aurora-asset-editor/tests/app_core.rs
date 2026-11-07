use std::fs;
use std::io;

use aurora_asset_editor::about::BuildInfo;
use aurora_asset_editor::app::{
    CurrentGame, LOG_WRITE_FAILED, Page, SETTINGS_LOAD_FAILED, SETTINGS_SAVE_FAILED,
    StartupProblem, StartupSettings,
};
use aurora_asset_editor::dialogs::{Modal, ModalQueue};
use aurora_asset_editor::settings::{APP_SETTINGS_FILE, ThemeChoice, WindowGeometry};
use aurora_asset_editor::status::{ActiveWork, ConsoleActivity, Locks, Severity, StatusLine};
use aurora_asset_editor::text::filter_hex_input;
use aurora_console::{GameIds, HexId};
use editor_test_support::AppHarness;

fn halo_3() -> CurrentGame {
    CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse("4D5307E6").unwrap(),
            database_id: HexId::from_database_value(18),
        },
    }
}

fn saved_settings_json(harness: &AppHarness) -> serde_json::Value {
    let text = fs::read_to_string(harness.config_dir().join(APP_SETTINGS_FILE)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn build_info(dirty: bool, run_url: Option<&str>) -> BuildInfo {
    BuildInfo {
        version: "2.0.7".to_owned(),
        short_hash: "a1b2c3d".to_owned(),
        full_hash: "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678".to_owned(),
        dirty,
        build_date: "2026-11-07".to_owned(),
        target: "x86_64-pc-windows-msvc".to_owned(),
        run_url: run_url.map(str::to_owned),
    }
}

fn message(title: &str) -> Modal {
    Modal::Message {
        title: title.to_owned(),
        text: format!("{title} text"),
    }
}

#[test]
fn locks_follow_active_work() {
    let editor_busy = Locks {
        editor: true,
        save_local: true,
        console_editor_writes: true,
        use_result: true,
        ..Locks::default()
    };
    let console_busy = Locks {
        console_actions: true,
        console_editor_writes: true,
        ..Locks::default()
    };
    let mut cases = vec![
        (ActiveWork::default(), Locks::default()),
        (
            ActiveWork {
                loading: 1,
                ..ActiveWork::default()
            },
            editor_busy,
        ),
        (
            ActiveWork {
                importing: 2,
                ..ActiveWork::default()
            },
            editor_busy,
        ),
        (
            ActiveWork {
                console: Some(ConsoleActivity::Downloading),
                ..ActiveWork::default()
            },
            Locks {
                console_actions: true,
                ..editor_busy
            },
        ),
        (
            ActiveWork {
                console: Some(ConsoleActivity::Bulk),
                ..ActiveWork::default()
            },
            Locks {
                bulk_stop: true,
                ..console_busy
            },
        ),
        (
            ActiveWork {
                online_search: true,
                ..ActiveWork::default()
            },
            Locks {
                online_search: true,
                ..Locks::default()
            },
        ),
        (
            ActiveWork {
                online_downloads: 3,
                ..ActiveWork::default()
            },
            Locks::default(),
        ),
        (
            ActiveWork {
                loading: 1,
                console: Some(ConsoleActivity::Bulk),
                online_search: true,
                ..ActiveWork::default()
            },
            Locks {
                editor: true,
                save_local: true,
                console_actions: true,
                console_editor_writes: true,
                online_search: true,
                use_result: true,
                bulk_stop: true,
            },
        ),
    ];
    for activity in [
        ConsoleActivity::Testing,
        ConsoleActivity::Refreshing,
        ConsoleActivity::Uploading,
        ConsoleActivity::Removing,
    ] {
        cases.push((
            ActiveWork {
                console: Some(activity),
                ..ActiveWork::default()
            },
            console_busy,
        ));
    }

    for (work, expected) in cases {
        assert_eq!(work.locks(), expected, "{work:?}");
    }
}

#[test]
fn starts_on_cover_with_ready_status() {
    let mut harness = AppHarness::new();
    let app = harness.app();

    assert_eq!(app.page(), Page::Cover);
    assert_eq!(
        app.status(),
        &StatusLine {
            text: "Ready".to_owned(),
            severity: Severity::Info,
            progress: None,
        }
    );
    assert_eq!(app.modal(), None);
    assert_eq!(app.current_game(), None);
    assert_eq!(app.locks(), Locks::default());
    assert_eq!(harness.error_log_text(), "");
}

#[test]
fn show_page_switches_pages() {
    let mut harness = AppHarness::new();

    harness.app().show_page(Page::Console);

    assert_eq!(harness.app().page(), Page::Console);
}

#[test]
fn current_game_chip_menu_copies_ids_and_clears() {
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo_3()));

    harness.app().copy_title_id();
    assert_eq!(harness.clipboard_text().as_deref(), Some("4D5307E6"));
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Copied TitleID 4D5307E6")
    );

    harness.app().copy_database_id();
    assert_eq!(harness.clipboard_text().as_deref(), Some("00000012"));
    assert_eq!(
        harness.app().status(),
        &StatusLine::info("Copied DatabaseID 00000012")
    );

    harness.app().clear_current_game();
    assert_eq!(harness.app().current_game(), None);
}

#[test]
fn clipboard_failure_is_logged_and_shown() {
    let mut harness = AppHarness::new();
    harness
        .clipboard()
        .fail_with("the clipboard is held by another program");
    harness.app().set_current_game(Some(halo_3()));

    harness.app().copy_title_id();

    assert_eq!(
        harness.app().status(),
        &StatusLine::error("Could not copy TitleID 4D5307E6: could not use the clipboard")
    );
    assert_eq!(harness.clipboard_text(), None);
    let log = harness.error_log_text();
    assert!(
        log.contains(
            "Could not copy TitleID 4D5307E6: could not use the clipboard\n  caused by: the clipboard is held by another program\n"
        ),
        "{log}"
    );
}

#[test]
fn error_log_write_failure_shows_in_status() {
    let mut harness = AppHarness::new();
    fs::create_dir(harness.config_dir().join("error.log")).unwrap();
    harness.clipboard().fail_with("busy");
    harness.app().set_current_game(Some(halo_3()));

    harness.app().copy_title_id();

    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    let expected_start = format!(
        "Could not copy TitleID 4D5307E6: could not use the clipboard · {LOG_WRITE_FAILED}: "
    );
    assert!(status.text.starts_with(&expected_start), "{}", status.text);
}

#[test]
fn toggling_settings_persists_them() {
    let mut harness = AppHarness::new();

    harness.app().set_auto_resize(false);
    assert!(!harness.app().settings().auto_resize);
    assert_eq!(saved_settings_json(&harness)["auto_resize"], false);

    harness.app().set_use_compression(false);
    assert!(!harness.app().settings().use_compression);
    assert_eq!(saved_settings_json(&harness)["use_compression"], false);

    harness.app().set_theme(ThemeChoice::Light);
    assert_eq!(harness.app().settings().theme, ThemeChoice::Light);
    assert_eq!(saved_settings_json(&harness)["theme"], "light");
    assert_eq!(harness.app().status(), &StatusLine::ready());
}

#[test]
fn settings_write_failure_is_reported() {
    let mut harness = AppHarness::new();
    fs::create_dir(harness.config_dir().join(APP_SETTINGS_FILE)).unwrap();

    harness.app().set_auto_resize(false);

    assert!(!harness.app().settings().auto_resize);
    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    let expected_start = format!("{SETTINGS_SAVE_FAILED}: could not write ");
    assert!(status.text.starts_with(&expected_start), "{}", status.text);
    let log = harness.error_log_text();
    assert!(log.contains(&expected_start), "{log}");
    assert!(log.contains("  caused by: "), "{log}");
}

#[test]
fn window_geometry_is_saved_with_the_settings() {
    let mut harness = AppHarness::new();
    let geometry = WindowGeometry {
        x: 120,
        y: 80,
        ..WindowGeometry::default()
    };

    harness.app().save_window_geometry(geometry);

    let json = saved_settings_json(&harness);
    assert_eq!(json["window"]["x"], 120);
    assert_eq!(json["window"]["y"], 80);
    assert_eq!(json["theme"], "dark");
}

#[test]
fn malformed_settings_file_is_kept_until_a_setting_changes() {
    let mut harness = AppHarness::new();
    let path = harness.config_dir().join(APP_SETTINGS_FILE);
    fs::write(&path, "{ not json").unwrap();
    let startup = StartupSettings::load(harness.config_dir());
    harness.with_startup_settings(startup);

    let status = harness.app().status().clone();
    assert_eq!(status.severity, Severity::Error);
    assert!(
        status
            .text
            .starts_with(&format!("{SETTINGS_LOAD_FAILED}: ")),
        "{}",
        status.text
    );
    assert!(
        status.text.ends_with("is not a valid settings file"),
        "{}",
        status.text
    );
    assert!(harness.error_log_text().contains(SETTINGS_LOAD_FAILED));
    assert!(harness.app().settings().use_compression);

    harness
        .app()
        .save_window_geometry(WindowGeometry::default());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");

    harness.app().set_use_compression(false);
    let json = saved_settings_json(&harness);
    assert_eq!(json["use_compression"], false);
    assert_eq!(json["auto_resize"], true);
}

#[test]
fn startup_problems_are_logged_and_shown() {
    let mut harness = AppHarness::new();
    harness.with_startup_problem(StartupProblem {
        context: "Could not find the config folder".to_owned(),
        error: Box::new(io::Error::other("no home folder")),
    });
    harness.with_startup_problem(StartupProblem {
        context: "Could not read the local address".to_owned(),
        error: Box::new(io::Error::other("no network adapter")),
    });

    assert_eq!(
        harness.app().status(),
        &StatusLine::error(
            "Could not find the config folder: no home folder (1 more in error.log)"
        )
    );
    let log = harness.error_log_text();
    assert!(
        log.contains("Could not find the config folder: no home folder\n"),
        "{log}"
    );
    assert!(
        log.contains("Could not read the local address: no network adapter\n"),
        "{log}"
    );
}

#[test]
fn modals_show_one_at_a_time() {
    let mut queue = ModalQueue::new();
    queue.push(message("First"));
    queue.push(message("Second"));

    assert_eq!(queue.current(), Some(&message("First")));
    assert_eq!(queue.close_current(), Some(message("First")));
    assert_eq!(queue.current(), Some(&message("Second")));
    assert_eq!(queue.close_current(), Some(message("Second")));
    assert_eq!(queue.current(), None);
    assert_eq!(queue.close_current(), None);
}

#[test]
fn build_pill_text_is_version_and_hash() {
    assert_eq!(build_info(false, None).pill_text(), "2.0.7 (a1b2c3d)");
}

#[test]
fn dirty_build_marks_the_hash() {
    assert_eq!(build_info(true, None).pill_text(), "2.0.7 (a1b2c3d.dirty)");
}

#[test]
fn window_title_names_version_and_credits() {
    assert_eq!(
        build_info(false, None).window_title(),
        "Aurora Asset Editor v2.0.7 by Swizzy thanks to MaesterRowen"
    );
}

#[test]
fn copy_text_has_five_lines_or_four_without_run_url() {
    let run_url = "https://github.com/Swizzy/AuroraAssetEditor/actions/runs/42";

    assert_eq!(
        build_info(true, Some(run_url)).copy_text(),
        "Aurora Asset Editor 2.0.7 (a1b2c3d.dirty)\n\
         Commit: a1b2c3d4e5f60718293a4b5c6d7e8f9012345678\n\
         Built: 2026-11-07\n\
         Target: x86_64-pc-windows-msvc\n\
         Run: https://github.com/Swizzy/AuroraAssetEditor/actions/runs/42"
    );
    assert_eq!(
        build_info(false, None).copy_text(),
        "Aurora Asset Editor 2.0.7 (a1b2c3d)\n\
         Commit: a1b2c3d4e5f60718293a4b5c6d7e8f9012345678\n\
         Built: 2026-11-07\n\
         Target: x86_64-pc-windows-msvc"
    );
}

#[test]
fn hex_input_filter_matches_old_box() {
    assert_eq!(filter_hex_input("4D53-07E6"), "4D5307E6");
    assert_eq!(filter_hex_input("zz12"), "12");
    assert_eq!(filter_hex_input("123456789"), "12345678");
    assert_eq!(filter_hex_input("4d5307e6"), "4d5307e6");
    assert_eq!(filter_hex_input(""), "");
}
