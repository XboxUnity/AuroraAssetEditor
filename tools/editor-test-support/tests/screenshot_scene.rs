use std::path::{MAIN_SEPARATOR, Path};
use std::thread;

use aurora_asset::AssetSlot;
use aurora_asset_editor::app::{CurrentGame, Menu, Page};
use aurora_asset_editor::jobs::{Job, OnlineJob};
use aurora_asset_editor::settings::ThemeChoice;
use aurora_console::{FtpSettings, GameIds, HexId};
use aurora_ui::ConsoleModel;
use editor_test_support::{
    AppHarness, HeadlessUi, STAGED_CONSOLE_HOST, Scene, ScreenTextProblem, check_screen_text,
};
use i_slint_backend_testing::ElementHandle;
use image::{Rgba, RgbaImage};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition};

const DARK_BAR: Rgba<u8> = Rgba([0x24, 0x27, 0x2e, 0xff]);
const LIGHT_BAR: Rgba<u8> = Rgba([0xff, 0xff, 0xff, 0xff]);
const TOP_BAR_PROBE: (u32, u32) = (500, 14);
const HALO_3: &str = "Halo 3";
const HALO_3_TITLE_ID: &str = "4D5307E6";
const NON_LATIN_TITLE: &str = "ポケモン";
/// A space Inter has no glyph for.
const IDEOGRAPHIC_SPACE: char = '\u{3000}';
/// Each top bar menu with the label of one of its rows.
const SHELL_MENUS: [(Menu, &str); 4] = [
    (Menu::Open, "Open asset files…"),
    (Menu::Save, "Save all to console…"),
    (Menu::Game, "Copy TitleID 4D5307E6"),
    (Menu::Gear, "About"),
];
/// Each Console page menu with the button it opens above; they share their rows.
const CONSOLE_MENUS: [(Menu, &str); 3] = [
    (Menu::Download, "ConsolePage::download-button"),
    (Menu::Upload, "ConsolePage::upload-button"),
    (Menu::Remove, "ConsolePage::remove-button"),
];
const CONSOLE_MENU_ROW: &str = "All";

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

fn workspace_folder() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("editor-test-support sits two folders below the workspace root")
}

fn assert_folder_rejected(folder: &Path, name: &str) {
    let path = folder.display().to_string();
    let scene = scene_with_game(&path);

    let problem = screen_text_problem(&scene);

    assert_names(&problem, name);
    assert_names(&problem, path.trim_end_matches(MAIN_SEPARATOR));
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
    for (menu, row_label) in SHELL_MENUS {
        scene.act(|app| app.toggle_menu(Some(menu)));
        assert_menu_copy_accepted(&scene, menu, row_label);
        scene.act(|app| app.toggle_menu(None));
    }

    scene.act(|app| app.show_page(Page::Console));
    for (menu, button_id) in CONSOLE_MENUS {
        scene.act(|app| app.toggle_menu(Some(menu)));
        open_above(&scene, button_id);
        assert_menu_copy_accepted(&scene, menu, CONSOLE_MENU_ROW);
        scene.act(|app| app.toggle_menu(None));
    }
}

/// Places the open Console menu above its button, as the button's click does; the buttons need a
/// connected console with a selected game, whose address would fail the check.
fn open_above(scene: &Scene, button_id: &str) {
    let anchor = scene.element(button_id).absolute_position();
    let console = scene.window().global::<ConsoleModel>();
    console.set_menu_anchor_x(anchor.x);
    console.set_menu_anchor_y(anchor.y);
}

fn assert_menu_copy_accepted(scene: &Scene, menu: Menu, row_label: &str) {
    assert!(
        ElementHandle::find_by_accessible_label(scene.window(), row_label)
            .next()
            .is_some(),
        "the {menu:?} menu shows its \"{row_label}\" row"
    );
    if let Err(problem) = check_screen_text(scene.window()) {
        panic!("{menu:?} menu: {problem}");
    }
}

#[test]
fn screen_text_check_rejects_a_local_address() {
    let scene = scene_with_game("127.0.0.1");

    assert_names(&screen_text_problem(&scene), "127.0.0.1");
}

#[test]
fn screen_text_check_rejects_an_address_other_than_the_staged_console() {
    let scene = scene_with_game("Console 192.168.1.77");

    assert_names(&screen_text_problem(&scene), "192.168.1.77");
}

#[test]
fn screen_text_check_accepts_the_staged_console() {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: STAGED_CONSOLE_HOST.to_owned(),
        port: "21".to_owned(),
        ..FtpSettings::default()
    });
    let scene = Scene::start(harness, ThemeChoice::Dark);
    scene.act(|app| app.show_page(Page::Console));

    assert!(
        ElementHandle::find_by_accessible_label(scene.window(), "192.168.1.50:21 · xboxftp")
            .next()
            .is_some(),
        "the console chip shows the staged address"
    );
    if let Err(problem) = check_screen_text(scene.window()) {
        panic!("{problem}");
    }
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
    assert_folder_rejected(&std::env::temp_dir(), "temp folder");
}

#[cfg(windows)]
#[test]
fn screen_text_check_rejects_an_upper_cased_temp_path_on_windows() {
    let upper_cased = std::env::temp_dir().display().to_string().to_uppercase();
    let scene = scene_with_game(&upper_cased);

    assert_names(&screen_text_problem(&scene), "temp folder");
}

#[test]
fn screen_text_check_rejects_the_workspace_path() {
    assert_folder_rejected(workspace_folder(), "workspace folder");
}

#[test]
fn screen_text_check_rejects_the_home_path() {
    let home = std::env::home_dir().expect("the test machine has a home folder");
    assert_folder_rejected(&home, "home folder");
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
fn screen_text_check_rejects_a_space_missing_from_inter() {
    let scene = scene_with_game(&format!("Halo{IDEOGRAPHIC_SPACE}3"));

    assert_names(
        &screen_text_problem(&scene),
        &format!("{IDEOGRAPHIC_SPACE:?}"),
    );
}

#[test]
#[should_panic(expected = "127.0.0.1")]
fn capture_fails_on_forbidden_text() {
    scene_with_game("127.0.0.1").capture();
}
