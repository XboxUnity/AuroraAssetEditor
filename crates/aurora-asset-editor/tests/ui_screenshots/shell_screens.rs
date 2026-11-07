use aurora_asset_editor::app::{Menu, SETTINGS_LOAD_FAILED, StartupProblem};
use aurora_asset_editor::settings::ThemeChoice;
use editor_test_support::{AppHarness, Scene, Screen, ScreenArea};

use crate::sample_content::{console_harness, halo_3};

const CLIPBOARD_FAILURE: &str = "clipboard is gone";
const SETTINGS_PARSE_ERROR: &str = "expected value at line 1 column 1";

pub const SHELL: ScreenArea = ScreenArea {
    title: "Shell",
    screens: &[
        Screen {
            name: "shell-open-menu",
            description: "The Open menu of an empty app.",
            stage: stage_open_menu,
        },
        Screen {
            name: "shell-save-menu-no-console",
            description: "The Save menu with no console set up, \"Save all to console…\" disabled with the reason \"No console set up\".",
            stage: stage_save_menu_no_console,
        },
        Screen {
            name: "shell-save-menu",
            description: "The Save menu with a console set up and a game selected, every row enabled.",
            stage: stage_save_menu,
        },
        Screen {
            name: "shell-game-menu",
            description: "The game chip's menu for Halo 3.",
            stage: stage_game_menu,
        },
        Screen {
            name: "shell-gear-menu",
            description: "The gear menu with \"Auto Resize Images\" and \"Use Compression\" both on.",
            stage: stage_gear_menu,
        },
        Screen {
            name: "shell-gear-menu-toggles-off",
            description: "The gear menu with \"Auto Resize Images\" and \"Use Compression\" both off.",
            stage: stage_gear_menu_toggles_off,
        },
        Screen {
            name: "shell-status-error",
            description: "The status line showing an error: the TitleID could not be copied because the clipboard failed.",
            stage: stage_status_error,
        },
        Screen {
            name: "shell-status-startup-problem",
            description: "The status line showing the problem found at start: the settings could not be loaded, so the defaults are in use.",
            stage: stage_status_startup_problem,
        },
    ],
};

fn menu_scene(harness: AppHarness, theme: ThemeChoice, menu: Menu) -> Scene {
    let scene = Scene::start(harness, theme);
    scene.act(|app| app.toggle_menu(Some(menu)));
    scene
}

fn stage_open_menu(theme: ThemeChoice) -> Scene {
    menu_scene(AppHarness::new(), theme, Menu::Open)
}

fn stage_save_menu_no_console(theme: ThemeChoice) -> Scene {
    menu_scene(AppHarness::new(), theme, Menu::Save)
}

fn stage_save_menu(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(console_harness(), theme);
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.toggle_menu(Some(Menu::Save));
    });
    scene
}

fn stage_game_menu(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.toggle_menu(Some(Menu::Game));
    });
    scene
}

fn stage_gear_menu(theme: ThemeChoice) -> Scene {
    menu_scene(AppHarness::new(), theme, Menu::Gear)
}

fn stage_gear_menu_toggles_off(theme: ThemeChoice) -> Scene {
    let scene = Scene::start(AppHarness::new(), theme);
    scene.act(|app| {
        app.set_auto_resize(false);
        app.set_use_compression(false);
        app.toggle_menu(Some(Menu::Gear));
    });
    scene
}

fn stage_status_error(theme: ThemeChoice) -> Scene {
    let mut scene = Scene::start(AppHarness::new(), theme);
    scene.harness().clipboard().fail_with(CLIPBOARD_FAILURE);
    scene.act(|app| {
        app.set_current_game(Some(halo_3()));
        app.copy_title_id();
    });
    scene
}

fn stage_status_startup_problem(theme: ThemeChoice) -> Scene {
    let mut harness = AppHarness::new();
    harness.with_startup_problem(StartupProblem {
        context: SETTINGS_LOAD_FAILED.to_owned(),
        error: SETTINGS_PARSE_ERROR.into(),
    });
    Scene::start(harness, theme)
}
