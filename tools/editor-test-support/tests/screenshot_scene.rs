use std::thread;

use aurora_asset::AssetSlot;
use aurora_asset_editor::app::{CurrentGame, Menu, Page};
use aurora_asset_editor::jobs::{Job, OnlineJob};
use aurora_asset_editor::settings::ThemeChoice;
use aurora_console::{GameIds, HexId};
use editor_test_support::{AppHarness, HeadlessUi, Scene, ScreenTextProblem, check_screen_text};
use i_slint_backend_testing::ElementHandle;
use image::{Rgba, RgbaImage};
use slint::LogicalPosition;
use slint::platform::WindowEvent;

const DARK_BAR: Rgba<u8> = Rgba([0x24, 0x27, 0x2e, 0xff]);
const LIGHT_BAR: Rgba<u8> = Rgba([0xff, 0xff, 0xff, 0xff]);
const TOP_BAR_PROBE: (u32, u32) = (500, 14);
const HALO_3: &str = "Halo 3";
const HALO_3_TITLE_ID: &str = "4D5307E6";
const NON_LATIN_TITLE: &str = "ポケモン";
const SHELL_MENUS: [Menu; 4] = [Menu::Open, Menu::Save, Menu::Game, Menu::Gear];
const CONSOLE_MENUS: [Menu; 3] = [Menu::Download, Menu::Upload, Menu::Remove];

fn game_titled(title: &str) -> CurrentGame {
    CurrentGame {
        title: title.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(HALO_3_TITLE_ID).expect("a valid TitleID"),
            database_id: HexId::from_database_value(0x12),
        },
    }
}

fn scene_with_game(title: &str) -> Scene {
    let scene = Scene::start(AppHarness::new(), ThemeChoice::Dark);
    scene.act(|app| app.set_current_game(Some(game_titled(title))));
    scene
}

/// Stages and captures on a fresh thread, as the suite does: Slint's platform and mock clock are per thread.
fn capture_on_own_thread(stage: impl FnOnce() -> RgbaImage + Send + 'static) -> RgbaImage {
    thread::spawn(stage)
        .join()
        .expect("staging the screen does not panic")
}

fn gear_menu_scene() -> Scene {
    let scene = scene_with_game(HALO_3);
    scene.click("TopBar::gear-button");
    scene
}

fn hover_about_row(scene: &Scene) {
    let about = ElementHandle::find_by_accessible_label(scene.window(), "About")
        .next()
        .expect("the gear menu has an About row");
    let (origin, size) = (about.absolute_position(), about.size());
    scene.dispatch(WindowEvent::PointerMoved {
        position: LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0),
    });
}

fn screen_text_problem(scene: &Scene) -> ScreenTextProblem {
    check_screen_text(scene.window()).expect_err("the screen shows machine-specific text")
}

fn assert_names(problem: &ScreenTextProblem, text: &str) {
    assert!(
        problem.problems.iter().any(|found| found.contains(text)),
        "a problem names {text}: {problem}"
    );
    assert!(problem.to_string().contains(text), "{problem}");
}

#[test]
fn take_next_job_hands_out_the_queued_job_without_running_it() {
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);
    harness.app().edit_search_text(HALO_3_TITLE_ID);
    harness.app().search_online();

    let job = harness.take_next_job();

    assert!(
        matches!(job, Some(Job::Online(OnlineJob::Search { .. }))),
        "{job:?}"
    );
    assert_eq!(harness.queued_jobs(), 0);
    assert_eq!(
        harness.app().status().text,
        format!("Searching XboxUnity for {HALO_3_TITLE_ID}…")
    );
}

#[test]
fn scene_renders_at_the_default_and_the_minimum_size() {
    let default = Scene::start(AppHarness::new(), ThemeChoice::Dark).capture();
    assert_eq!(default.dimensions(), HeadlessUi::WINDOW_SIZE);
    assert_eq!(HeadlessUi::WINDOW_SIZE, (1024, 680));

    let minimum = Scene::start_at_minimum_size(AppHarness::new(), ThemeChoice::Dark).capture();
    assert_eq!(minimum.dimensions(), HeadlessUi::MINIMUM_WINDOW_SIZE);
    assert_eq!(HeadlessUi::MINIMUM_WINDOW_SIZE, (760, 500));
}

#[test]
fn capture_applies_the_theme() {
    for (theme, bar) in [
        (ThemeChoice::Dark, DARK_BAR),
        (ThemeChoice::Light, LIGHT_BAR),
    ] {
        let capture = Scene::start(AppHarness::new(), theme).capture();
        let (x, y) = TOP_BAR_PROBE;
        assert_eq!(*capture.get_pixel(x, y), bar, "{theme:?} top bar");
    }
}

#[test]
fn capture_is_identical_when_staged_twice() {
    let first = capture_on_own_thread(|| gear_menu_scene().capture());
    let second = capture_on_own_thread(|| gear_menu_scene().capture());

    assert!(first == second, "two stagings of one screen differ");
}

#[test]
fn capture_parks_the_pointer() {
    let untouched = capture_on_own_thread(|| gear_menu_scene().capture());
    let hovered_then_parked = capture_on_own_thread(|| {
        let scene = gear_menu_scene();
        hover_about_row(&scene);
        scene.capture()
    });
    let hovered_and_kept = capture_on_own_thread(|| {
        let mut scene = gear_menu_scene();
        hover_about_row(&scene);
        scene.keep_pointer_in_place();
        scene.capture()
    });

    assert!(
        hovered_then_parked == untouched,
        "the capture parks the pointer outside the window"
    );
    assert!(
        hovered_and_kept != untouched,
        "the About row has a hover look while the pointer stays on it"
    );
}

#[test]
fn screen_text_check_accepts_the_ui_copy() {
    let scene = scene_with_game(HALO_3);
    for (page, menus) in [
        (Page::Cover, &SHELL_MENUS[..]),
        (Page::Console, &CONSOLE_MENUS[..]),
    ] {
        scene.act(|app| app.show_page(page));
        for menu in menus {
            scene.act(|app| app.toggle_menu(Some(*menu)));
            if let Err(problem) = check_screen_text(scene.window()) {
                panic!("{menu:?} menu: {problem}");
            }
            scene.act(|app| app.toggle_menu(None));
        }
    }
}

#[test]
fn screen_text_check_rejects_a_local_address() {
    let scene = scene_with_game("127.0.0.1");

    assert_names(&screen_text_problem(&scene), "127.0.0.1");
}

#[test]
fn screen_text_check_rejects_an_os_error_text() {
    let mut scene = scene_with_game(HALO_3);
    scene
        .harness()
        .dialogs()
        .push_failure("Access is denied. (os error 5)");
    scene.act(|app| app.replace_image(AssetSlot::Cover));

    assert_names(&screen_text_problem(&scene), "os error");
}

#[test]
fn screen_text_check_rejects_a_temp_path() {
    let temp_folder = std::env::temp_dir().display().to_string();
    let scene = scene_with_game(&temp_folder);

    let problem = screen_text_problem(&scene);

    assert_names(&problem, "temp folder");
    assert_names(
        &problem,
        temp_folder.trim_end_matches(std::path::MAIN_SEPARATOR),
    );
}

#[test]
fn screen_text_check_rejects_a_glyph_missing_from_inter() {
    let scene = scene_with_game(NON_LATIN_TITLE);

    let problem = screen_text_problem(&scene);

    assert_names(&problem, NON_LATIN_TITLE);
    for character in NON_LATIN_TITLE.chars() {
        assert_names(&problem, &format!("{character:?}"));
    }
}

#[test]
#[should_panic(expected = "127.0.0.1")]
fn capture_fails_on_forbidden_text() {
    scene_with_game("127.0.0.1").capture();
}
