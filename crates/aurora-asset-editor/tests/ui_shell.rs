use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use aurora_asset::{AssetSlot, Compression, ScreenshotNumber};
use aurora_asset_editor::app::{App, CurrentGame, Menu, Page};
use aurora_asset_editor::dialogs::{Modal, ModalAnswer};
use aurora_asset_editor::platform::SystemTheme;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::{ImageCache, MainWindowView};
use aurora_console::{GameIds, HexId};
use aurora_ui::{MainWindow, MenuId, ShellModel};
use editor_test_support::{AppHarness, HeadlessUi, asset_file_with};
use i_slint_backend_testing::ElementHandle;
use image::{Rgba, RgbaImage};
use slint::ComponentHandle;
use slint::platform::{Key, PointerEventButton, WindowEvent};

const DARK_BAR: Rgba<u8> = Rgba([0x24, 0x27, 0x2e, 0xff]);
const LIGHT_BAR: Rgba<u8> = Rgba([0xff, 0xff, 0xff, 0xff]);
const DARK_ERROR_TEXT: Rgba<u8> = Rgba([0xf8, 0x71, 0x71, 0xff]);
const LIGHT_ERROR_TEXT: Rgba<u8> = Rgba([0xb9, 0x1c, 0x1c, 0xff]);
const DARK_STATUS_BACKGROUND: Rgba<u8> = Rgba([0x20, 0x23, 0x29, 0xff]);
const LIGHT_STATUS_BACKGROUND: Rgba<u8> = Rgba([0xee, 0xf0, 0xf3, 0xff]);
const COLOUR_TOLERANCE: u8 = 24;
const TOP_BAR_PROBE: (u32, u32) = (500, 14);
/// Right of the sidebar, below the top bar and above the status line.
const BODY_AREA: (u32, u32, u32, u32) = (140, 40, 880, 600);
/// The status line's text, left of the build pill.
const STATUS_AREA: (u32, u32, u32, u32) = (0, 654, 700, 26);
const ALL_PAGES: [Page; 6] = [
    Page::Cover,
    Page::Background,
    Page::IconBanner,
    Page::Screenshots,
    Page::Online,
    Page::Console,
];
/// Set to a folder to keep the rendered shells as PNG files for a look.
const SNAPSHOT_DIR_VARIABLE: &str = "AURORA_UI_SNAPSHOT_DIR";

struct Shell {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Shell {
    fn start(harness: &mut AppHarness) -> Shell {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window);
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let shell = Shell { ui, view, app };
        shell.render();
        shell
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }

    fn act(&self, action: impl FnOnce(&mut App)) {
        action(&mut self.app.borrow_mut());
        self.render();
    }

    fn snapshot(&self) -> RgbaImage {
        self.ui.snapshot(self.view.window().window())
    }

    fn element(&self, id: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(self.view.window(), id)
            .next()
            .unwrap_or_else(|| panic!("the window has an element {id}"))
    }

    fn model(&self) -> ShellModel<'_> {
        self.view.window().global::<ShellModel>()
    }
}

fn harness_with_theme(theme: ThemeChoice) -> AppHarness {
    let mut harness = AppHarness::new();
    harness.app().set_theme(theme);
    harness
}

fn colour_count(image: &RgbaImage) -> usize {
    image.pixels().collect::<HashSet<_>>().len()
}

fn crop(image: &RgbaImage, (x, y, width, height): (u32, u32, u32, u32)) -> RgbaImage {
    image::imageops::crop_imm(image, x, y, width, height).to_image()
}

/// The pixel that differs most from the background: the fully covered part of a glyph.
fn strongest_text_pixel(image: &RgbaImage, background: Rgba<u8>) -> Rgba<u8> {
    *image
        .pixels()
        .max_by_key(|pixel| distance(**pixel, background))
        .expect("the area has pixels")
}

fn distance(first: Rgba<u8>, second: Rgba<u8>) -> u32 {
    first
        .0
        .iter()
        .zip(second.0)
        .map(|(a, b)| u32::from(a.abs_diff(b)))
        .sum()
}

/// Anti-aliased 11 px glyphs do not quite reach the text colour.
fn near(pixel: Rgba<u8>, expected: Rgba<u8>) -> bool {
    pixel
        .0
        .iter()
        .zip(expected.0)
        .all(|(a, b)| a.abs_diff(b) <= COLOUR_TOLERANCE)
}

fn top_bar_pixel(shell: &Shell) -> Rgba<u8> {
    let (x, y) = TOP_BAR_PROBE;
    *shell.snapshot().get_pixel(x, y)
}

fn keep_snapshot(image: &RgbaImage, name: &str) {
    if let Some(folder) = std::env::var_os(SNAPSHOT_DIR_VARIABLE) {
        let folder = PathBuf::from(folder);
        std::fs::create_dir_all(&folder).expect("the snapshot folder can be created");
        image
            .save(folder.join(format!("{name}.png")))
            .expect("the snapshot can be written");
    }
}

fn halo() -> CurrentGame {
    CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse("4D5307E6").expect("a valid TitleID"),
            database_id: HexId::from_database_value(0x12),
        },
    }
}

fn load_three_screenshots(harness: &mut AppHarness) {
    let folder = tempfile::tempdir().expect("tests need a temp folder");
    let image = RgbaImage::from_pixel(8, 8, Rgba([0, 128, 255, 255]));
    let entries: Vec<_> = ScreenshotNumber::ALL[..3]
        .iter()
        .map(|number| {
            (
                AssetSlot::Screenshot(*number),
                image.clone(),
                Compression::None,
            )
        })
        .collect();
    let path = folder.path().join("SS4D5307E6.asset");
    std::fs::write(&path, asset_file_with(&entries).to_bytes())
        .expect("the fixture can be written");
    harness.app().open_paths(vec![path]);
    harness.run_jobs();
}

/// What names the page in the body: the asset pages' panel titles, or the Online and Console pages' main
/// buttons.
fn body_titles(shell: &Shell) -> Vec<String> {
    [
        "ImagePanel::title",
        "OnlinePage::search-button",
        "ConsolePage::refresh-button",
    ]
    .into_iter()
    .flat_map(|id| ElementHandle::find_by_element_id(shell.view.window(), id))
    .filter_map(|title| title.accessible_label())
    .map(|title| title.to_string())
    .collect()
}

fn page_titles(page: Page) -> Vec<&'static str> {
    match page {
        Page::Cover => vec!["Cover"],
        Page::Background => vec!["Background"],
        Page::IconBanner => vec!["Icon", "Banner"],
        Page::Screenshots => vec!["Screenshot 1"],
        Page::Online => vec!["Search"],
        Page::Console => vec!["Refresh"],
    }
}

#[test]
fn shell_renders_every_page_in_both_themes() {
    for (theme, bar, name) in [
        (ThemeChoice::Dark, DARK_BAR, "dark"),
        (ThemeChoice::Light, LIGHT_BAR, "light"),
    ] {
        let mut harness = harness_with_theme(theme);
        load_three_screenshots(&mut harness);
        harness.app().set_current_game(Some(halo()));
        let shell = Shell::start(&mut harness);
        let mut bodies = Vec::new();
        for page in ALL_PAGES {
            shell.act(|app| app.show_page(page));
            let snapshot = shell.snapshot();
            keep_snapshot(&snapshot, &format!("shell-{name}-{page:?}"));

            assert!(
                colour_count(&snapshot) > 1,
                "{name} {page:?} renders more than one colour"
            );
            let (x, y) = TOP_BAR_PROBE;
            assert_eq!(*snapshot.get_pixel(x, y), bar, "{name} top bar");
            assert_eq!(
                body_titles(&shell),
                page_titles(page),
                "{name} {page:?} shows its own page"
            );
            bodies.push(crop(&snapshot, BODY_AREA));
        }
        for (index, body) in bodies.iter().enumerate() {
            for other in &bodies[index + 1..] {
                assert_ne!(body, other, "{name}: every page has its own body");
            }
        }
    }
}

#[test]
fn sidebar_shows_screenshot_count_when_above_zero() {
    let mut empty = AppHarness::new();
    let shell = Shell::start(&mut empty);
    assert_eq!(shell.model().get_screenshot_count(), 0);

    let mut harness = AppHarness::new();
    load_three_screenshots(&mut harness);
    let shell = Shell::start(&mut harness);
    assert_eq!(shell.model().get_screenshot_count(), 3);
}

#[test]
fn a_job_run_after_the_view_is_connected_updates_the_view() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);
    assert_eq!(shell.model().get_screenshot_count(), 0);

    load_three_screenshots(&mut harness);
    shell.render();

    assert_eq!(shell.model().get_screenshot_count(), 3);
    assert_eq!(shell.model().get_page(), aurora_ui::PageId::Screenshots);
    assert_eq!(
        shell
            .element("StatusLine::status-text")
            .accessible_label()
            .as_deref(),
        Some("Loaded SS4D5307E6.asset · 1 of 5 asset types present")
    );
}

#[test]
fn sidebar_click_switches_page() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);

    shell
        .element("SideBar::console-item")
        .mock_single_click(PointerEventButton::Left);

    assert_eq!(shell.app.borrow().page(), Page::Console);
    assert_eq!(shell.model().get_page(), aurora_ui::PageId::Console);
}

#[test]
fn menus_open_from_state_and_act() {
    let mut harness = AppHarness::new();
    load_three_screenshots(&mut harness);
    let shell = Shell::start(&mut harness);
    let closed = shell.snapshot();

    shell.act(|app| app.toggle_menu(Some(Menu::Open)));
    let open = shell.snapshot();
    assert_eq!(shell.model().get_open_menu(), MenuId::Open);
    assert_ne!(open, closed);

    shell
        .element("OpenMenu::new-asset-set-row")
        .invoke_accessible_default_action();

    assert_eq!(shell.app.borrow().pages().present_types(), 0);
    assert_eq!(shell.app.borrow().open_menu(), None);
    assert_eq!(shell.model().get_open_menu(), MenuId::None);

    shell.act(|app| app.toggle_menu(Some(Menu::Gear)));
    assert_eq!(shell.model().get_open_menu(), MenuId::Gear);
}

#[test]
fn clicking_outside_an_open_menu_closes_it() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);

    shell
        .element("TopBar::open-button")
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(shell.app.borrow().open_menu(), Some(Menu::Open));

    shell
        .element("SideBar::cover-item")
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(shell.app.borrow().open_menu(), None);
    assert_eq!(shell.app.borrow().page(), Page::Cover);
}

#[test]
fn game_chip_shows_ids_and_copies() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);
    assert_eq!(
        shell
            .element("TopBar::game-chip")
            .accessible_label()
            .as_deref(),
        Some("no game selected")
    );
    assert_eq!(
        shell.element("TopBar::game-chip").accessible_enabled(),
        Some(false)
    );

    shell.act(|app| app.set_current_game(Some(halo())));
    let chip = shell.element("TopBar::game-chip");
    assert_eq!(
        chip.accessible_label().as_deref(),
        Some("Halo 3 · 4D5307E6 · DB 00000012")
    );
    chip.mock_single_click(PointerEventButton::Left);
    assert_eq!(shell.app.borrow().open_menu(), Some(Menu::Game));

    let copy_title_id = shell.element("GameMenu::copy-title-id-row");
    assert_eq!(
        copy_title_id.accessible_label().as_deref(),
        Some("Copy TitleID 4D5307E6")
    );
    assert_eq!(
        shell
            .element("GameMenu::copy-database-id-row")
            .accessible_label()
            .as_deref(),
        Some("Copy DatabaseID 00000012")
    );
    copy_title_id.invoke_accessible_default_action();
    assert_eq!(harness.clipboard_text().as_deref(), Some("4D5307E6"));
    assert_eq!(shell.app.borrow().open_menu(), None);

    shell.act(|app| app.toggle_menu(Some(Menu::Game)));
    shell
        .element("GameMenu::clear-selection-row")
        .invoke_accessible_default_action();
    assert_eq!(shell.app.borrow().current_game(), None);
    assert_eq!(
        shell
            .element("TopBar::game-chip")
            .accessible_label()
            .as_deref(),
        Some("no game selected")
    );
}

#[test]
fn build_pill_shows_pill_text() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);

    assert_eq!(
        shell
            .element("StatusLine::build-pill")
            .accessible_label()
            .as_deref(),
        Some("2.0.7 (a1b2c3d)")
    );
    assert_eq!(
        shell.model().get_window_title(),
        "Aurora Asset Editor v2.0.7 by Swizzy thanks to MaesterRowen"
    );
}

#[test]
fn build_pill_and_gear_about_show_the_about_dialog() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);

    shell
        .element("StatusLine::build-pill")
        .mock_single_click(PointerEventButton::Left);
    assert!(matches!(shell.app.borrow().modal(), Some(Modal::About)));
    shell.app.borrow_mut().answer_modal(ModalAnswer::Accept);
    assert!(shell.app.borrow().modal().is_none());
    shell.view.render(&shell.app.borrow());

    shell
        .element("TopBar::gear-button")
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(shell.app.borrow().open_menu(), Some(Menu::Gear));
    let about = ElementHandle::find_by_accessible_label(shell.view.window(), "About")
        .next()
        .expect("the gear menu has an About item");
    about.invoke_accessible_default_action();
    assert!(matches!(shell.app.borrow().modal(), Some(Modal::About)));
    assert_eq!(shell.app.borrow().open_menu(), None);
}

#[test]
fn image_cache_converts_only_changed_revisions() {
    let image = |colour: [u8; 4]| {
        slint::Image::from_rgba8(
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&colour[..], 1, 1),
        )
    };
    let first_pixel = |image: &slint::Image| image.to_rgba8().map(|buffer| buffer.as_slice()[0]);
    let red = image([255, 0, 0, 255]);
    let blue = image([0, 0, 255, 255]);
    let mut cache = ImageCache::default();

    let shown = cache.image(AssetSlot::Cover, 1, || red.clone());
    assert_eq!(first_pixel(&shown), first_pixel(&red));
    let again = cache.image(AssetSlot::Cover, 1, || {
        panic!("revision 1 is converted once")
    });
    assert_eq!(first_pixel(&again), first_pixel(&red));
    let other_slot = cache.image(AssetSlot::Background, 1, || blue.clone());
    assert_eq!(first_pixel(&other_slot), first_pixel(&blue));
    let changed = cache.image(AssetSlot::Cover, 2, || blue.clone());
    assert_eq!(first_pixel(&changed), first_pixel(&blue));
}

#[test]
fn render_converts_only_slots_whose_revision_changed() {
    let mut harness = AppHarness::new();
    load_three_screenshots(&mut harness);
    let shell = Shell::start(&mut harness);
    let editor = shell.view.window().global::<aurora_ui::EditorModel>();
    let first_tile = || {
        use slint::Model;
        editor
            .get_screenshot_tiles()
            .row_data(0)
            .expect("tile 1 is shown")
            .image
    };
    let shown = first_tile();

    // Images compare by buffer, so an equal image was not converted again.
    shell.act(|app| app.set_drop_hover(true));
    assert_eq!(
        first_tile(),
        shown,
        "an unrelated change reuses the converted image"
    );

    shell.act(|app| {
        app.move_screenshot(
            ScreenshotNumber::new(1).expect("position 1"),
            ScreenshotNumber::new(2).expect("position 2"),
        )
    });
    assert_ne!(
        first_tile(),
        shown,
        "the moved positions are converted again"
    );
}

#[test]
fn status_line_shows_errors_in_error_colour() {
    for (theme, background, error_text) in [
        (ThemeChoice::Dark, DARK_STATUS_BACKGROUND, DARK_ERROR_TEXT),
        (
            ThemeChoice::Light,
            LIGHT_STATUS_BACKGROUND,
            LIGHT_ERROR_TEXT,
        ),
    ] {
        let mut harness = harness_with_theme(theme);
        harness.app().set_current_game(Some(halo()));
        let shell = Shell::start(&mut harness);
        let status = shell.element("StatusLine::status-text");
        assert_eq!(status.accessible_label().as_deref(), Some("Ready"));
        let ready = strongest_text_pixel(&crop(&shell.snapshot(), STATUS_AREA), background);
        assert!(
            !near(ready, error_text),
            "{theme:?}: Ready is not drawn in the error colour"
        );

        harness.clipboard().fail_with("clipboard is gone");
        shell.act(|app| app.copy_title_id());

        let text = status
            .accessible_label()
            .expect("the status text has a label");
        assert!(
            text.starts_with("Could not copy TitleID 4D5307E6"),
            "{text}"
        );
        assert!(shell.model().get_status_is_error());
        let error = strongest_text_pixel(&crop(&shell.snapshot(), STATUS_AREA), background);
        assert!(
            near(error, error_text),
            "{theme:?}: {error:?} is the error colour"
        );
    }
}

#[test]
fn save_to_console_item_shows_reason_when_not_set_up() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);
    let reason = shell
        .app
        .borrow()
        .save_to_console_disabled_reason()
        .expect("no console is set up");

    shell
        .element("TopBar::save-button")
        .mock_single_click(PointerEventButton::Left);
    let row = shell.element("SaveMenu::save-to-console-row");

    assert_eq!(
        row.accessible_label().as_deref(),
        Some("Save all to console…")
    );
    assert_eq!(row.accessible_description().as_deref(), Some(reason));
    assert_eq!(row.accessible_enabled(), Some(false));
}

#[test]
fn theme_choice_drives_palette() {
    for system_dark in [false, true] {
        for (theme, expected) in [
            (ThemeChoice::Dark, DARK_BAR),
            (ThemeChoice::Light, LIGHT_BAR),
        ] {
            let mut harness = harness_with_theme(theme);
            let shell = Shell::start(&mut harness);
            shell
                .view
                .set_system_theme(SystemTheme { dark: system_dark });
            shell.render();
            assert_eq!(
                top_bar_pixel(&shell),
                expected,
                "{theme:?}, system dark {system_dark}"
            );
        }
    }

    let mut harness = harness_with_theme(ThemeChoice::System);
    let shell = Shell::start(&mut harness);
    for (system_dark, expected) in [(true, DARK_BAR), (false, LIGHT_BAR)] {
        shell
            .view
            .set_system_theme(SystemTheme { dark: system_dark });
        shell.render();
        assert_eq!(
            top_bar_pixel(&shell),
            expected,
            "System, system dark {system_dark}"
        );
    }
}

#[test]
fn rendering_writes_the_theme_to_the_std_palette() {
    let mut harness = harness_with_theme(ThemeChoice::Light);
    let shell = Shell::start(&mut harness);
    // Change handlers run on the next event loop turn or frame; the snapshot draws a frame.
    shell.snapshot();
    assert_eq!(
        shell.model().get_palette_color_scheme(),
        slint::language::ColorScheme::Light
    );

    shell.act(|app| app.set_theme(ThemeChoice::Dark));
    shell.snapshot();
    assert_eq!(
        shell.model().get_palette_color_scheme(),
        slint::language::ColorScheme::Dark
    );
}

#[test]
fn system_theme_reads_the_palette_before_the_first_render() {
    let ui = HeadlessUi::new();
    let window = MainWindow::new().expect("the main window builds on the testing backend");
    ui.show(window.window());
    let mut theme = SystemTheme::read_initial(&window);
    assert!(!theme.dark, "the testing backend reports no dark OS theme");

    theme.update(true);
    assert!(theme.dark);
}

#[test]
fn shell_sizes_follow_the_mockup() {
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo()));
    let shell = Shell::start(&mut harness);
    let height = |id: &str| shell.element(id).size().height;

    assert_eq!(height("MainWindow::top-bar"), 33.0);
    assert_eq!(height("MainWindow::status-line"), 24.0);
    assert_eq!(shell.element("MainWindow::side-bar").size().width, 135.0);
    for row in [
        "SideBar::cover-item",
        "SideBar::screenshots-item",
        "SideBar::console-item",
    ] {
        assert_eq!(height(row), 26.0, "{row}");
    }
    for pill in [
        "TopBar::open-button",
        "TopBar::save-button",
        "TopBar::game-chip",
        "TopBar::gear-button",
    ] {
        assert_eq!(height(pill), 20.0, "{pill}");
    }
}

#[test]
fn escape_closes_an_open_menu() {
    let mut harness = AppHarness::new();
    let shell = Shell::start(&mut harness);
    shell
        .element("TopBar::gear-button")
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(shell.app.borrow().open_menu(), Some(Menu::Gear));

    let escape = slint::SharedString::from(Key::Escape);
    let window = shell.view.window().window();
    window.dispatch_event(WindowEvent::KeyPressed {
        text: escape.clone(),
    });
    window.dispatch_event(WindowEvent::KeyReleased { text: escape });

    assert_eq!(shell.app.borrow().open_menu(), None);
}

#[test]
fn every_menu_renders_in_both_themes() {
    for (theme, name) in [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")] {
        let mut harness = harness_with_theme(theme);
        harness.app().set_current_game(Some(halo()));
        let shell = Shell::start(&mut harness);
        let closed = shell.snapshot();
        for (menu, menu_name) in [
            (Menu::Open, "open"),
            (Menu::Save, "save"),
            (Menu::Game, "game"),
            (Menu::Gear, "gear"),
        ] {
            shell.act(|app| app.toggle_menu(Some(menu)));
            let open = shell.snapshot();
            keep_snapshot(&open, &format!("shell-{name}-{menu_name}-menu"));
            assert_ne!(open, closed, "{name} {menu_name} menu");
            shell.act(|app| app.toggle_menu(None));
        }
    }
}
