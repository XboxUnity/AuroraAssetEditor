use aurora_asset_editor::app::Page;
use aurora_asset_editor::console::GameColumn;
use aurora_asset_editor::dialogs::ModalAnswer;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_console::{AssetPresence, ConsoleError};
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};

use crate::sample_content::{console_harness, png_file, sample_art, sample_games};
use crate::staged_outcomes::{mark_presence, refresh_fails, report_bulk_progress, show_games};

const TABLE: &str = "ConsolePage::game-table";
const SELECTED_GAME: &str = "Fable II";
const FIRST_GAME: usize = 0;
const SECOND_GAME: usize = 1;
const THIRD_GAME: usize = 2;
const TITLE_FILTER: &str = "a";
const TITLE_ID_FILTER: &str = "4D53";
/// Counted from 1, as the status line shows it: the run is at the second sample game.
const BULK_POSITION: usize = 2;
const DROPPED_IMAGE: &str = "art.png";
const DROPPED_IMAGE_SIZE: (u32, u32) = (64, 64);
const DROPPED_IMAGE_SEED: u8 = 5;
/// The third sample game's presence check fails with an error that is not a lost connection, so the
/// status and the link stay as they are while its cell shows the failure.
const UNREADABLE_FOLDER: &str = "list 58410A1C_00000012";

pub const CONSOLE: ScreenArea = ScreenArea {
    title: "Console",
    screens: &[
        Screen {
            name: "console-not-set-up",
            description: "The Console page with no connection set up: the \"Not set up\" chip, the actions disabled because no game is selected.",
            stage: stage_not_set_up,
        },
        Screen {
            name: "console-refreshing",
            description: "A refresh is running: the status says \"Refreshing the game list…\", Refresh and Bulk actions… are disabled and so are the console actions (no game is selected either); the filters and the table stay usable.",
            stage: stage_refreshing,
        },
        Screen {
            name: "console-games",
            description: "The eight sample games with the link connected, the assets column not filled in yet and the status \"Connected · 8 games · last refresh 20:41\".",
            stage: stage_games,
        },
        Screen {
            name: "console-presence",
            description: "Asset marks in the table: Halo 3 has every kind, Fable II the cover only, Hexic HD could not be checked, the rest are unknown.",
            stage: stage_presence,
        },
        Screen {
            name: "console-selected",
            description: "The second row selected: the game chip reads \"Fable II · 4D530842 · DB 00000002\" and the actions are enabled.",
            stage: stage_selected,
        },
        Screen {
            name: "console-filtered",
            description: "The title filter \"a\" and the TitleID filter \"4D53\" applied together.",
            stage: stage_filtered,
        },
        Screen {
            name: "console-sorted",
            description: "The table sorted by TitleID, descending.",
            stage: stage_sorted,
        },
        Screen {
            name: "console-download-menu",
            description: "The Download menu open above its button, with a game selected.",
            stage: stage_download_menu,
        },
        Screen {
            name: "console-upload-menu",
            description: "The Upload current menu open above its button, with a game selected.",
            stage: stage_upload_menu,
        },
        Screen {
            name: "console-remove-menu",
            description: "The Remove menu open above its button, with a game selected.",
            stage: stage_remove_menu,
        },
        Screen {
            name: "console-refresh-failed",
            description: "A refresh that failed because the console has no Content.db with any content: the status line shows the error and the connection chip is as it was.",
            stage: stage_refresh_failed,
        },
        Screen {
            name: "console-refresh-not-set-up",
            description: "Refresh pressed with no connection set up: the status line asks for \"Edit connection…\" first.",
            stage: stage_refresh_not_set_up,
        },
        Screen {
            name: "console-image-dropped",
            description: "An image dropped on the Console page: the status line says where images can be dropped.",
            stage: stage_image_dropped,
        },
        Screen {
            name: "console-bulk-running",
            description: "A bulk cover run at game 2 of 8: the status shows its progress with Stop, Refresh and Bulk actions… are disabled and so are the console actions (no game is selected either); the filters and the table stay usable.",
            stage: stage_bulk_running,
        },
    ],
};

/// The Console page of an app with the console set up.
fn start_console(theme: ThemeChoice) -> Scene {
    start_console_with(console_harness(), theme)
}

fn start_console_with(harness: AppHarness, theme: ThemeChoice) -> Scene {
    let scene = Scene::start(harness, theme);
    scene.act(|app| app.show_page(Page::Console));
    scene
}

/// The Console page listing the sample games.
fn start_with_games(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| show_games(app, sample_games()));
    scene
}

/// The sample games with the presence of the first three rows answered.
fn start_with_presence(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme);
    let games = sample_games();
    scene.act(|app| {
        mark_presence(app, &games[FIRST_GAME].ids, Ok(every_kind()));
        mark_presence(app, &games[SECOND_GAME].ids, Ok(cover_only()));
        mark_presence(
            app,
            &games[THIRD_GAME].ids,
            Err(ConsoleError::DataConnectionFailed {
                action: UNREADABLE_FOLDER.to_owned(),
            }),
        );
    });
    scene
}

/// The presence marks shown, then the second row clicked as a person selects a game.
fn start_with_selection(theme: ThemeChoice) -> Scene {
    let scene = start_with_presence(theme);
    scene.click_label_in(SELECTED_GAME, TABLE);
    scene
}

fn open_menu_by_clicking(theme: ThemeChoice, button: &str) -> Scene {
    let scene = start_with_selection(theme);
    scene.click(button);
    scene
}

fn stage_not_set_up(theme: ThemeChoice) -> Scene {
    start_console_with(AppHarness::new(), theme)
}

fn stage_refreshing(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| app.refresh_games());
    scene
}

fn stage_games(theme: ThemeChoice) -> Scene {
    start_with_games(theme)
}

fn stage_presence(theme: ThemeChoice) -> Scene {
    start_with_presence(theme)
}

fn stage_selected(theme: ThemeChoice) -> Scene {
    start_with_selection(theme)
}

fn stage_filtered(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme);
    scene.act(|app| app.set_game_filter(TITLE_FILTER, TITLE_ID_FILTER));
    scene
}

fn stage_sorted(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme);
    scene.act(|app| {
        app.sort_games(GameColumn::TitleId);
        app.sort_games(GameColumn::TitleId);
    });
    scene
}

fn stage_download_menu(theme: ThemeChoice) -> Scene {
    open_menu_by_clicking(theme, "ConsolePage::download-button")
}

fn stage_upload_menu(theme: ThemeChoice) -> Scene {
    open_menu_by_clicking(theme, "ConsolePage::upload-button")
}

fn stage_remove_menu(theme: ThemeChoice) -> Scene {
    open_menu_by_clicking(theme, "ConsolePage::remove-button")
}

fn stage_refresh_failed(theme: ThemeChoice) -> Scene {
    let scene = start_console(theme);
    scene.act(|app| refresh_fails(app, ConsoleError::ContentDbMissing));
    scene
}

fn stage_refresh_not_set_up(theme: ThemeChoice) -> Scene {
    let scene = start_console_with(AppHarness::new(), theme);
    scene.act(|app| app.refresh_games());
    scene
}

fn stage_image_dropped(theme: ThemeChoice) -> Scene {
    let mut scene = start_with_games(theme);
    let image = sample_art(
        DROPPED_IMAGE_SIZE.0,
        DROPPED_IMAGE_SIZE.1,
        DROPPED_IMAGE_SEED,
    );
    let path = png_file(&mut scene, DROPPED_IMAGE, &image);
    scene.act(|app| app.files_dropped(vec![path]));
    scene
}

fn stage_bulk_running(theme: ThemeChoice) -> Scene {
    let scene = start_with_games(theme);
    let games = sample_games();
    let current_title = &games[BULK_POSITION - 1].title_name;
    scene.act(|app| {
        app.open_bulk_actions();
        app.answer_modal(ModalAnswer::Accept);
        report_bulk_progress(app, BULK_POSITION, games.len(), current_title);
    });
    scene
}

fn every_kind() -> AssetPresence {
    AssetPresence {
        cover: true,
        background: true,
        icon_banner: true,
        screenshots: true,
    }
}

fn cover_only() -> AssetPresence {
    AssetPresence {
        cover: true,
        ..AssetPresence::NONE
    }
}
