use aurora_asset_editor::app::{KindChoice, Page};
use aurora_asset_editor::dialogs::ModalAnswer;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_console::{BulkEntry, BulkFailure, BulkOutcome, BulkReport, ConsoleError, Game};
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};
use slint::platform::Key;

use crate::sample_content::{console_harness, sample_games};
use crate::staged_outcomes::{
    answer_connection_test, fail_running_refresh, finish_bulk, refresh_fails, sample_revision,
    show_games,
};

/// The sample game Halo 3, the current game of the catalogue.
const HALO_3_ROW: usize = 0;
const BULK_FINISHED_GAMES: usize = 4;
/// The code a console answers a wrong username or password with.
const LOGIN_REJECTED_CODE: u32 = 530;
const PICKER_FILTER: &str = "GamePicker::filter-field";
/// Part of the TitleIDs of Fable II, Gears of War 2 and Forza Motorsport 3.
const TITLE_ID_FILTER: &str = "4D5308";
const HALO_FILTER: &str = "halo";

pub const CONSOLE_DIALOGS: ScreenArea = ScreenArea {
    title: "Console dialogs",
    screens: &[
        Screen {
            name: "dialog-upload-target-loading",
            description: "\"Save all assets to console\" opened with no game selected while it lists the console's games: \"Refreshing the game list…\" in the picker, both ID boxes empty, OK disabled.",
            stage: stage_upload_target_loading,
        },
        Screen {
            name: "dialog-upload-target-filtered",
            description: "\"Save all assets to console\" over the listed sample games with 4D5308 typed in the picker: the three games whose TitleID holds it, the first highlighted.",
            stage: stage_upload_target_filtered,
        },
        Screen {
            name: "dialog-upload-target-filled",
            description: "\"Save all assets to console\" after Return picked Halo 3, the match of halo typed in the picker: its TitleID and DatabaseID in the boxes, OK enabled.",
            stage: stage_upload_target_filled,
        },
        Screen {
            name: "dialog-upload-target-manual",
            description: "\"Save all assets to console\" after listing the console's games failed: only the ID boxes, the error in the status line and in a red toast; no \"Connection failed\" dialog waits behind it.",
            stage: stage_upload_target_manual,
        },
        Screen {
            name: "dialog-remove-confirm",
            description: "\"Remove from console?\" for all of Halo 3's assets, with Remove in the danger colour.",
            stage: stage_remove_confirm,
        },
        Screen {
            name: "dialog-upload-empty-confirm",
            description: "\"Upload empty assets?\" after uploading all four pages while the editor holds none.",
            stage: stage_upload_empty_confirm,
        },
        Screen {
            name: "dialog-connection-defaults",
            description: "\"Console connection\" with no settings saved: the IP box holds the local subnet, the port 21 and the user xboxftp.",
            stage: stage_connection_defaults,
        },
        Screen {
            name: "dialog-connection-test-passed",
            description: "\"Console connection\" after a test that found Aurora revision 1655: \"Connection to Aurora revision 1655 Established...\".",
            stage: stage_connection_test_passed,
        },
        Screen {
            name: "dialog-connection-test-failed",
            description: "\"Console connection\" after a test the console rejected the login for: the error in the error colour.",
            stage: stage_connection_test_failed,
        },
        Screen {
            name: "dialog-connection-failed",
            description: "\"Connection failed\" over the Console page after a refresh the console rejected the login for.",
            stage: stage_connection_failed,
        },
        Screen {
            name: "dialog-bulk",
            description: "\"Bulk actions\" over the eight sample games: source XboxUnity, Replace existing off, the text counting 8 games.",
            stage: stage_bulk,
        },
        Screen {
            name: "dialog-bulk-no-games",
            description: "\"Bulk actions\" with the text \"No Assets listed\", opened while the table lists no games.",
            stage: stage_bulk_no_games,
        },
        Screen {
            name: "dialog-bulk-finished",
            description: "\"Bulk actions finished\" with the summary of a four-game run: one saved to the console, one already had a cover, one without a cover online, one failed.",
            stage: stage_bulk_finished,
        },
    ],
};

fn start_console(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| app.show_page(Page::Console));
    scene
}

fn start_with_games(theme: ThemeChoice, games: Vec<Game>) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| show_games(app, games));
    scene
}

fn start_with_halo_3_selected(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme, sample_games());
    scene.act(|app| app.select_game(HALO_3_ROW));
    scene
}

fn start_with_connection_dialog(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| app.edit_connection());
    scene
}

/// The dialog opens without a current game: with one, Save all to console asks nothing.
fn stage_upload_target_loading(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| app.save_all_to_console());
    scene
}

fn upload_target_with_filter(theme: ThemeChoice, filter: &str) -> Scene {
    let scene = start_with_games(theme, sample_games());
    scene.act(|app| app.save_all_to_console());
    scene.click(PICKER_FILTER);
    scene.type_text(filter);
    scene
}

fn stage_upload_target_filtered(theme: ThemeChoice) -> Scene {
    upload_target_with_filter(theme, TITLE_ID_FILTER)
}

fn stage_upload_target_filled(theme: ThemeChoice) -> Scene {
    let scene = upload_target_with_filter(theme, HALO_FILTER);
    scene.press(Key::Return);
    scene
}

fn stage_upload_target_manual(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| {
        app.save_all_to_console();
        fail_running_refresh(app, login_rejected());
    });
    scene
}

fn stage_remove_confirm(theme: ThemeChoice) -> Scene {
    let scene = start_with_halo_3_selected(theme);
    scene.act(|app| app.remove_from_console(KindChoice::All));
    scene
}

fn stage_upload_empty_confirm(theme: ThemeChoice) -> Scene {
    let scene = start_with_halo_3_selected(theme);
    scene.act(|app| app.upload_current(KindChoice::All));
    scene
}

fn stage_connection_defaults(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| app.edit_connection());
    scene
}

fn stage_connection_test_passed(theme: ThemeChoice) -> Scene {
    let mut scene = start_with_connection_dialog(theme);
    scene.act(|app| app.test_connection());
    answer_connection_test(&mut scene, Ok(sample_revision()));
    scene
}

fn stage_connection_test_failed(theme: ThemeChoice) -> Scene {
    let mut scene = start_with_connection_dialog(theme);
    scene.act(|app| app.test_connection());
    answer_connection_test(&mut scene, Err(login_rejected()));
    scene
}

fn stage_connection_failed(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| refresh_fails(app, login_rejected()));
    scene
}

fn stage_bulk(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme, sample_games());
    scene.act(|app| app.open_bulk_actions());
    scene
}

fn stage_bulk_no_games(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| app.open_bulk_actions());
    scene
}

/// A four-game run is started through the dialog, then ended with its report.
fn stage_bulk_finished(theme: ThemeChoice) -> Scene {
    let games: Vec<Game> = sample_games()
        .into_iter()
        .take(BULK_FINISHED_GAMES)
        .collect();
    let scene = start_with_games(theme, games.clone());
    scene.act(|app| {
        app.open_bulk_actions();
        app.answer_modal(ModalAnswer::Accept);
        finish_bulk(app, four_game_report(games));
    });
    scene
}

fn four_game_report(games: Vec<Game>) -> BulkReport {
    let outcomes = [
        BulkOutcome::Uploaded,
        BulkOutcome::AlreadyHasCover,
        BulkOutcome::NoCoverFound,
        BulkOutcome::Failed(BulkFailure::Console(ConsoleError::TooManyConnections)),
    ];
    let total = games.len();
    BulkReport {
        entries: games
            .into_iter()
            .zip(outcomes)
            .map(|(game, outcome)| BulkEntry { game, outcome })
            .collect(),
        total,
        stopped: None,
    }
}

fn login_rejected() -> ConsoleError {
    ConsoleError::LoginRejected {
        code: LOGIN_REJECTED_CODE,
    }
}
