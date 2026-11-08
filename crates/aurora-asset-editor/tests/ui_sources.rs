use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::path::PathBuf;
use std::rc::Rc;

use aurora_asset::{AssetKind, AssetSlot, Compression, ScreenshotNumber};
use aurora_asset_editor::app::{App, BUSY_TEXT, Menu, NO_CONSOLE_TEXT, Page, SELECT_A_GAME_REASON};
use aurora_asset_editor::console::{GameColumn, PresenceCell};
use aurora_asset_editor::dialogs::ModalAnswer;
use aurora_asset_editor::jobs::{ConsoleOutcome, FailedJob, Job, JobOutcome, JobPanic, OnlineJob};
use aurora_asset_editor::online::{SearchMode, TileImage};
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::{MainWindowView, OnlineImageCache, ResultImage, ResultImageKey};
use aurora_console::{
    AssetPresence, ConsoleError, FtpSettings, Game, GameIds, GameListing, HexId, asset_file_name,
    asset_folder, read_content_db,
};
use aurora_online::{CoverSource, Endpoints};
use aurora_ui::{ConsoleModel, GameRowView, MainWindow, OnlineModel, ShellModel};
use editor_test_support::{AppHarness, HeadlessUi, asset_file_with};
use i_slint_backend_testing::ElementHandle;
use image::RgbaImage;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, SharedString};
use test_servers::{FakeConsole, FakeConsoleConfig, RecordedHttp, Route, RouteBehaviour};

const ARCHIVE_PATH: &str = "/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/";
const RESULTS_TITLE: &str = "0000A001";
const NO_RESULTS_TITLE: &str = "0000A404";
const FAILING_TITLE: &str = "0000A500";
const EMPTY_GRID_TEXT: &str = "Search XboxUnity or the Internet Archive for covers";
const FULL_WIDTH: u32 = 90;
const FULL_HEIGHT: u32 = 60;
const LONG_TITLE_LENGTH: usize = 200;
const MANY_GAMES: usize = 60;
/// Inside a button's left padding, clear of its rounded corners.
const ACCENT_PROBE_INSET: f32 = 3.0;
/// The accent #3b82f6 has blue far above red; the neutral pill colours do not.
const MAX_NEUTRAL_BLUE_CAST: u8 = 20;
/// WCAG AA for text below 18 pt.
const MIN_TEXT_CONTRAST: f64 = 4.5;
/// Half the height of the rows searched for a centred caption's pixels.
const TEXT_BAND_HALF_HEIGHT: f32 = 6.0;
/// Inside an element's left edge, clear of its border and of its text.
const BACKGROUND_PROBE_INSET: f32 = 3.0;
const THEMES: [(ThemeChoice, &str); 2] =
    [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")];

struct Ui {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Ui {
    fn start(harness: &mut AppHarness) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window);
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let started = Ui { ui, view, app };
        started.render();
        started
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
            .unwrap_or_else(|| panic!("the window shows {id}"))
    }

    fn labelled(&self, label: &str) -> ElementHandle {
        ElementHandle::find_by_accessible_label(self.view.window(), label)
            .next()
            .unwrap_or_else(|| panic!("the window shows {label}"))
    }

    fn click(&self, id: &str) {
        self.element(id).mock_single_click(PointerEventButton::Left);
    }

    fn click_label(&self, label: &str) {
        self.labelled(label)
            .mock_single_click(PointerEventButton::Left);
    }

    /// The element labelled `label` that lies inside the element `container`.
    fn labelled_in(&self, label: &str, container: &str) -> ElementHandle {
        let container = self.element(container);
        let (origin, size) = (container.absolute_position(), container.size());
        ElementHandle::find_by_accessible_label(self.view.window(), label)
            .find(|element| {
                let position = element.absolute_position();
                position.x >= origin.x
                    && position.y >= origin.y
                    && position.x < origin.x + size.width
                    && position.y < origin.y + size.height
            })
            .unwrap_or_else(|| panic!("{label} is inside the container"))
    }

    fn click_label_in(&self, label: &str, container: &str) {
        self.labelled_in(label, container)
            .mock_single_click(PointerEventButton::Left);
    }

    fn enabled(&self, id: &str) -> bool {
        self.element(id)
            .accessible_enabled()
            .expect("buttons report whether they are enabled")
    }

    fn press(&self, key: Key) {
        let text = SharedString::from(key);
        let window = self.view.window().window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
    }

    fn online(&self) -> OnlineModel<'_> {
        self.view.window().global::<OnlineModel>()
    }

    fn console(&self) -> ConsoleModel<'_> {
        self.view.window().global::<ConsoleModel>()
    }

    fn rows(&self) -> Vec<GameRowView> {
        self.console().get_rows().iter().collect()
    }

    fn row_titles(&self) -> Vec<String> {
        self.rows()
            .iter()
            .map(|row| row.title.to_string())
            .collect()
    }
}

/// A disabled accent button is drawn like a disabled neutral one: no blue cast at its left padding.
fn assert_not_accent_coloured(ui: &Ui, id: &str) {
    let button = ui.element(id);
    assert_eq!(button.accessible_enabled(), Some(false), "{id} is disabled");
    let (position, size) = (button.absolute_position(), button.size());
    let snapshot = ui.snapshot();
    let pixel = snapshot.get_pixel(
        (position.x + ACCENT_PROBE_INSET) as u32,
        (position.y + size.height / 2.0) as u32,
    );
    let [red, _, blue, _] = pixel.0;
    assert!(
        blue.saturating_sub(red) < MAX_NEUTRAL_BLUE_CAST,
        "{id} is drawn neutral, not in the accent colour: {pixel:?}"
    );
}

fn assert_all_differ(states: &[(&str, RgbaImage)], theme: &str) {
    for (index, (name, image)) in states.iter().enumerate() {
        for (other_name, other) in &states[index + 1..] {
            assert_ne!(
                image, other,
                "{theme}: {name} and {other_name} look the same"
            );
        }
    }
}

// Online

fn png(width: u32, height: u32, shade: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([shade, (x * 2 % 251) as u8, (y * 3 % 251) as u8, 255])
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

fn image_route(path: &str, width: u32, height: u32, shade: u8) -> Route {
    route(path, 200, "image/png", png(width, height, shade))
}

fn gone_route(path: &str) -> Route {
    route(path, 404, "text/plain", b"gone".to_vec())
}

/// XboxUnity's answer for RESULTS_TITLE: covers One and Two load, Broken's thumbnail and image are gone.
fn result_routes() -> Vec<Route> {
    let entry = |name: &str| {
        let lower = name.to_lowercase();
        format!(
            r#"{{"name":"{name}","official":true,"rating":"5","url":"{{server}}/image/{lower}.png","thumbnail":"{{server}}/thumb/{lower}.png","titleid":"{RESULTS_TITLE}"}}"#
        )
    };
    let listing = format!("[{},{},{}]", entry("One"), entry("Two"), entry("Broken"));
    vec![
        route(
            &format!("/api/Covers/{RESULTS_TITLE}"),
            200,
            "application/json",
            listing.into_bytes(),
        ),
        image_route("/thumb/one.png", 30, 20, 200),
        image_route("/thumb/two.png", 30, 20, 120),
        gone_route("/thumb/broken.png"),
        image_route("/image/one.png", FULL_WIDTH, FULL_HEIGHT, 230),
        image_route("/image/two.png", FULL_WIDTH, FULL_HEIGHT, 90),
        gone_route("/image/broken.png"),
    ]
}

fn online_server() -> RecordedHttp {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/online");
    RecordedHttp::from_recordings_with_local_images(
        &fixtures.join("recordings.toml"),
        result_routes(),
    )
}

fn online_harness(server: &RecordedHttp, theme: ThemeChoice) -> AppHarness {
    let base = server.base_url();
    let mut harness = AppHarness::new();
    harness.with_online(Endpoints {
        xboxunity_covers: format!("{base}/api/Covers/"),
        archive_covers: format!("{base}{ARCHIVE_PATH}"),
    });
    harness.app().set_theme(theme);
    harness.app().show_page(Page::Online);
    harness
}

#[test]
fn online_page_renders_states() {
    let server = online_server();
    for (theme, name) in THEMES {
        let mut harness = online_harness(&server, theme);
        let ui = Ui::start(&mut harness);
        let mut states = Vec::new();

        assert_eq!(ui.online().get_empty_text(), EMPTY_GRID_TEXT);
        assert_not_accent_coloured(&ui, "OnlinePage::use-button");
        states.push(("empty", ui.snapshot()));

        ui.act(|app| {
            app.edit_search_text(RESULTS_TITLE);
            app.search_online();
        });
        assert_eq!(
            ui.online().get_empty_text(),
            "",
            "the hint goes while searching"
        );
        states.push(("searching", ui.snapshot()));

        harness.run_jobs();
        ui.render();
        let tiles: Vec<_> = ui.online().get_tiles().iter().collect();
        assert_eq!(tiles.len(), 3);
        assert!(tiles[0].has_image && tiles[1].has_image, "thumbnails show");
        assert!(
            tiles[2].failed && !tiles[2].has_image,
            "the broken thumbnail fails"
        );
        assert_eq!(ui.online().get_selected(), -1);
        assert!(!ui.enabled("OnlinePage::use-button"));
        states.push(("results", ui.snapshot()));

        ui.click_label_in(&tiles[0].label, "OnlinePage::result-grid");
        harness.run_jobs();
        ui.render();
        assert_eq!(ui.online().get_selected(), 0);
        assert!(ui.online().get_selected_has_image());
        assert_eq!(
            ui.online().get_selected_size(),
            format!("{FULL_WIDTH}x{FULL_HEIGHT}")
        );
        assert_eq!(ui.online().get_selected_label(), tiles[0].label);
        assert!(ui.enabled("OnlinePage::use-button"));
        assert!(ui.enabled("OnlinePage::save-button"));
        states.push(("selected", ui.snapshot()));

        ui.click_label_in(&tiles[2].label, "OnlinePage::result-grid");
        harness.run_jobs();
        ui.render();
        assert!(matches!(
            harness.app().online().results[2].full,
            TileImage::Failed
        ));
        assert!(ui.online().get_selected_failed());
        assert!(!ui.enabled("OnlinePage::use-button"));
        states.push(("failed", ui.snapshot()));

        assert_all_differ(&states, name);
    }
}

#[test]
fn image_failures_are_readable_on_the_selected_background() {
    let server = online_server();
    for (theme, name) in THEMES {
        let mut harness = online_harness(&server, theme);
        let ui = Ui::start(&mut harness);
        ui.act(|app| {
            app.edit_search_text(RESULTS_TITLE);
            app.search_online();
        });
        harness.run_jobs();
        ui.render();
        let broken = ui.online().get_tiles().row_data(2).expect("a third result");
        ui.click_label_in(&broken.label, "OnlinePage::result-grid");
        harness.run_jobs();
        ui.render();
        assert!(ui.online().get_selected_failed());

        let snapshot = ui.snapshot();
        let tile = ui.labelled_in(&broken.label, "OnlinePage::result-grid");
        let preview = ui.element("OnlinePage::preview");
        for (place, element) in [("tile", tile), ("preview", preview)] {
            let contrast = centre_text_contrast(&snapshot, &element);
            assert!(
                contrast >= MIN_TEXT_CONTRAST,
                "{name}: the failure on the selected {place} has contrast {contrast:.2}"
            );
        }
    }
}

/// The highest contrast between a pixel of the text centred in `element` and the background at its left.
fn centre_text_contrast(snapshot: &RgbaImage, element: &ElementHandle) -> f64 {
    let (position, size) = (element.absolute_position(), element.size());
    let centre_y = position.y + size.height / 2.0;
    let background = *snapshot.get_pixel(
        (position.x + BACKGROUND_PROBE_INSET) as u32,
        centre_y as u32,
    );
    let rows = (centre_y - TEXT_BAND_HALF_HEIGHT) as u32..(centre_y + TEXT_BAND_HALF_HEIGHT) as u32;
    let columns = position.x as u32..(position.x + size.width) as u32;
    rows.flat_map(|y| columns.clone().map(move |x| (x, y)))
        .map(|(x, y)| contrast_ratio(*snapshot.get_pixel(x, y), background))
        .fold(1.0, f64::max)
}

/// The WCAG contrast ratio of two colours.
fn contrast_ratio(first: image::Rgba<u8>, second: image::Rgba<u8>) -> f64 {
    let (first, second) = (relative_luminance(first), relative_luminance(second));
    let (lighter, darker) = (first.max(second), first.min(second));
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(colour: image::Rgba<u8>) -> f64 {
    let [red, green, blue, _] = colour.0.map(|channel| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

#[test]
fn a_search_that_found_nothing_or_failed_leaves_its_status_in_the_grid() {
    let server = RecordedHttp::start(vec![
        route(
            &format!("/api/Covers/{NO_RESULTS_TITLE}"),
            200,
            "application/json",
            b"[]".to_vec(),
        ),
        route(
            &format!("/api/Covers/{FAILING_TITLE}"),
            500,
            "text/plain",
            b"broken".to_vec(),
        ),
    ]);
    let mut harness = online_harness(&server, ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);

    for title in [NO_RESULTS_TITLE, FAILING_TITLE] {
        ui.act(|app| {
            app.edit_search_text(title);
            app.search_online();
        });
        harness.run_jobs();
        ui.render();

        let status = harness.app().status().text.clone();
        assert_eq!(
            ui.online().get_empty_text(),
            status,
            "after searching {title}"
        );
    }
}

#[test]
fn a_search_job_that_stopped_leaves_its_status_in_the_grid() {
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.edit_search_text(RESULTS_TITLE);
        app.search_online();
    });
    let job = harness.take_next_job();
    let Some(Job::Online(OnlineJob::Search { generation, .. })) = job else {
        panic!("the next job is not an online search: {job:?}");
    };

    ui.act(|app| {
        app.on_job_outcome(JobOutcome::Failed {
            job: FailedJob::OnlineSearch { generation },
            panic: JobPanic::from_payload(&"the search broke"),
        });
    });

    let status = harness.app().status().text.clone();
    assert_eq!(ui.online().get_empty_text(), status);
}

#[test]
fn use_as_cover_from_the_panel_sets_the_cover() {
    let server = online_server();
    let mut harness = online_harness(&server, ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.edit_search_text(RESULTS_TITLE);
        app.search_online();
    });
    harness.run_jobs();
    ui.render();
    let first = ui.online().get_tiles().row_data(0).expect("a result").label;
    ui.click_label_in(&first, "OnlinePage::result-grid");
    harness.run_jobs();
    ui.render();

    ui.click("OnlinePage::use-button");
    harness.run_jobs();
    ui.render();

    let size = harness
        .app()
        .pages()
        .cover
        .texture_header(AssetSlot::Cover)
        .map(|header| (header.width(), header.height()));
    assert_eq!(size, Some((FULL_WIDTH, FULL_HEIGHT)), "never resized");
    assert_eq!(harness.app().page(), Page::Online);
}

#[test]
fn search_is_disabled_while_searching_any_source() {
    let server = online_server();
    for source in [CoverSource::XboxUnity, CoverSource::InternetArchive] {
        let mut harness = online_harness(&server, ThemeChoice::Dark);
        harness.app().set_online_source(source);
        let ui = Ui::start(&mut harness);
        ui.click("TextField::input");
        ui.ui.type_text(ui.view.window().window(), RESULTS_TITLE);
        assert!(ui.enabled("OnlinePage::search-button"), "{source:?}");

        ui.click("OnlinePage::search-button");
        ui.render();
        assert_eq!(harness.queued_jobs(), 1, "{source:?}: one search");
        assert!(!ui.enabled("OnlinePage::search-button"), "{source:?}");

        ui.click("OnlinePage::search-button");
        ui.press(Key::Return);
        ui.act(|app| app.search_online());
        assert_eq!(
            harness.queued_jobs(),
            1,
            "{source:?}: the button, Enter and the action add no search"
        );
        assert_eq!(harness.app().status().text, BUSY_TEXT, "{source:?}");

        harness.run_jobs();
        ui.render();
        assert!(ui.enabled("OnlinePage::search-button"), "{source:?}");
        ui.press(Key::Return);
        assert_eq!(harness.queued_jobs(), 1, "{source:?}: Enter searches again");
    }
}

#[test]
fn search_mode_switch_disables_keywords_for_archive() {
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);
    let ui = Ui::start(&mut harness);

    ui.click_label("Keywords");
    assert_eq!(harness.app().online().mode, SearchMode::Keywords);
    assert_eq!(ui.online().get_mode_index(), 1);
    assert_eq!(ui.online().get_search_placeholder(), "Keywords");

    ui.click_label("Internet Archive");
    assert_eq!(harness.app().online().source, CoverSource::InternetArchive);
    assert_eq!(harness.app().online().mode, SearchMode::TitleId);
    assert!(!ui.online().get_keywords_enabled());
    assert_eq!(ui.labelled("Keywords").accessible_enabled(), Some(false));

    ui.click_label("Keywords");
    assert_eq!(
        harness.app().online().mode,
        SearchMode::TitleId,
        "Keywords stays off"
    );
    assert_eq!(ui.online().get_search_placeholder(), "TitleID");

    ui.click_label("XboxUnity");
    assert!(ui.online().get_keywords_enabled());
    assert_eq!(ui.labelled("Keywords").accessible_enabled(), Some(true));
}

#[test]
fn title_id_input_filters_typed_text() {
    let mut harness = AppHarness::new();
    harness.app().show_page(Page::Online);
    let ui = Ui::start(&mut harness);
    ui.click("TextField::input");

    ui.ui.type_text(ui.view.window().window(), "4D53-07E6");

    assert_eq!(harness.app().online().title_id_text, "4D5307E6");
    assert_eq!(ui.online().get_search_text(), "4D5307E6");
    let field = ui.element("TextField::input");
    assert_eq!(field.accessible_value().as_deref(), Some("4D5307E6"));

    ui.ui.type_text(ui.view.window().window(), "99");
    assert_eq!(
        harness.app().online().title_id_text,
        "4D5307E6",
        "at most eight digits"
    );
    assert_eq!(field.accessible_value().as_deref(), Some("4D5307E6"));
}

// Console

fn game(title: &str, title_id: &str, database_id: i64, disc: u32) -> Game {
    Game {
        title_name: title.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(title_id).expect("a valid TitleID"),
            database_id: HexId::from_database_value(database_id),
        },
        media_id: HexId::parse("1A2B3C4D").expect("a valid MediaID"),
        disc,
    }
}

fn mixed_games() -> Vec<Game> {
    vec![
        game("banana", "4D5307E6", 1, 2),
        game("Apple", "4D5307E7", 0x12, 10),
        game("cherry", "4E5307E6", 3, 1),
    ]
}

/// A harness whose app has connection settings that no test connects to.
fn console_harness(theme: ThemeChoice) -> AppHarness {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: "192.168.1.50".to_owned(),
        port: "21".to_owned(),
        ..FtpSettings::default()
    });
    harness.app().set_theme(theme);
    harness.app().show_page(Page::Console);
    harness
}

/// Fills the table as a refresh would, without a console.
fn show_games(app: &mut App, games: Vec<Game>) {
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

fn typical_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/console/content-db/typical.db")
}

fn typical_console() -> FakeConsole {
    let typical = std::fs::read(typical_db_path()).expect("the sample database is readable");
    FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical)
}

/// Puts an asset file of `kind` for `ids` on the console.
fn console_has(console: &FakeConsole, ids: &GameIds, kind: AssetKind) {
    let slot = match kind {
        AssetKind::Cover => AssetSlot::Cover,
        AssetKind::Background => AssetSlot::Background,
        AssetKind::IconBanner => AssetSlot::Icon,
        AssetKind::Screenshots => AssetSlot::Screenshot(ScreenshotNumber::ALL[0]),
    };
    let image = RgbaImage::from_pixel(8, 8, image::Rgba([10, 40, 90, 255]));
    let mut file = asset_file_with(&[(slot, image, Compression::None)]);
    let path = format!(
        "{}/{}",
        asset_folder(ids),
        asset_file_name(kind, &ids.title_id)
    );
    console.add_file(&path, &file.to_bytes());
}

/// The typical console, with every kind for its first game and a cover for its second.
fn console_with_assets() -> FakeConsole {
    let console = typical_console();
    let games = read_content_db(&typical_db_path()).expect("the sample database reads");
    for kind in AssetKind::ALL {
        console_has(&console, &games[0].ids, kind);
    }
    console_has(&console, &games[1].ids, AssetKind::Cover);
    console
}

#[test]
fn console_page_renders_states() {
    for (theme, name) in THEMES {
        let mut states = Vec::new();

        let mut not_set_up = AppHarness::new();
        not_set_up.app().set_theme(theme);
        not_set_up.app().show_page(Page::Console);
        let ui = Ui::start(&mut not_set_up);
        assert_eq!(ui.console().get_chip_text(), "Not set up");
        assert_eq!(ui.console().get_link_state(), 0);
        assert_eq!(ui.console().get_actions_reason(), SELECT_A_GAME_REASON);
        assert_not_accent_coloured(&ui, "ConsolePage::open-in-editor-button");
        assert!(!ui.enabled("ConsolePage::download-button"));
        states.push(("not-set-up", ui.snapshot()));
        ui.click("ConsolePage::refresh-button");
        assert_eq!(not_set_up.app().status().text, NO_CONSOLE_TEXT);
        drop(ui);

        let console = console_with_assets();
        let mut harness = AppHarness::new();
        harness.with_console(&console);
        harness.app().set_theme(theme);
        harness.app().show_page(Page::Console);
        let ui = Ui::start(&mut harness);
        ui.click("ConsolePage::refresh-button");
        harness.run_jobs();
        ui.render();
        assert_eq!(ui.console().get_link_state(), 2, "connected");
        assert_eq!(
            ui.console().get_chip_text(),
            format!("127.0.0.1:{} · xboxftp", console.port())
        );
        let rows = ui.rows();
        assert!(!rows.is_empty());
        for row in &rows {
            for id in [&row.title_id, &row.database_id, &row.media_id] {
                assert_eq!(id.len(), 8, "{id} is shown as eight hex digits");
            }
        }
        let marks: Vec<_> = rows
            .iter()
            .map(|row| {
                let marks = &row.marks;
                (
                    marks.cover,
                    marks.background,
                    marks.icon_banner,
                    marks.screenshots,
                )
            })
            .collect();
        assert_eq!(marks[0], (1, 1, 1, 1), "every kind is on the console");
        assert_eq!(marks[1], (1, 2, 2, 2), "only the cover is on the console");
        assert!(marks[2..].iter().all(|marks| *marks == (2, 2, 2, 2)));
        states.push(("games", ui.snapshot()));

        ui.click_label_in(&rows[0].title, "ConsolePage::game-table");
        assert!(ui.enabled("ConsolePage::download-button"));
        ui.click("ConsolePage::download-button");
        assert_eq!(harness.app().open_menu(), Some(Menu::Download));
        states.push(("menu", ui.snapshot()));
        ui.act(|app| app.toggle_menu(None));

        ui.click("ConsolePage::bulk-button");
        ui.act(|app| app.answer_modal(ModalAnswer::Accept));
        assert!(
            ui.view
                .window()
                .global::<ShellModel>()
                .get_bulk_stop_available()
        );
        assert!(ui.console().get_console_busy());
        assert!(!ui.enabled("ConsolePage::refresh-button"));
        assert!(!ui.enabled("ConsolePage::download-button"));
        ui.labelled("Stop");
        states.push(("bulk-running", ui.snapshot()));

        assert_all_differ(&states, name);
    }
}

#[test]
fn transfers_that_touch_the_editor_wait_while_it_is_busy() {
    let folder = tempfile::TempDir::new().expect("tests need a temp folder");
    let image = folder.path().join("cover.png");
    RgbaImage::from_pixel(8, 8, image::Rgba([10, 40, 90, 255]))
        .save(&image)
        .expect("the PNG can be written");
    let mut harness = console_harness(ThemeChoice::Dark);
    harness.dialogs().push_files(vec![image]);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    ui.click_label_in("banana", "ConsolePage::game-table");
    let editor_buttons = [
        "ConsolePage::open-in-editor-button",
        "ConsolePage::download-button",
        "ConsolePage::upload-button",
    ];
    for button in editor_buttons {
        assert!(ui.enabled(button), "{button} is enabled");
    }

    ui.act(|app| app.replace_image(AssetSlot::Cover));
    assert!(harness.app().locks().editor, "the import is queued");
    for button in editor_buttons {
        assert!(!ui.enabled(button), "{button} waits for the import");
    }
    assert!(
        ui.enabled("ConsolePage::remove-button"),
        "Remove leaves the editor alone"
    );
}

#[test]
fn game_actions_need_the_selected_row_on_screen() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    let game_buttons = [
        "ConsolePage::open-in-editor-button",
        "ConsolePage::download-button",
        "ConsolePage::upload-button",
        "ConsolePage::remove-button",
    ];
    ui.click_label_in("banana", "ConsolePage::game-table");
    for button in game_buttons {
        assert!(ui.enabled(button), "{button} is enabled");
    }

    ui.act(|app| app.set_game_filter("cherry", ""));
    for button in game_buttons {
        assert!(
            !ui.enabled(button),
            "{button}: the selected row is filtered out"
        );
    }
    assert_eq!(ui.console().get_actions_reason(), SELECT_A_GAME_REASON);
    assert!(
        harness.app().current_game().is_some(),
        "the game chip still names the game"
    );

    ui.act(|app| app.set_game_filter("", ""));
    ui.click_label_in("banana", "ConsolePage::game-table");
    for button in game_buttons {
        assert!(ui.enabled(button), "{button} is enabled again");
    }
}

#[test]
fn presence_marks_follow_the_presence_cells() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    assert!(
        ui.rows().iter().all(|row| row.marks.cover == 0),
        "not checked yet"
    );

    let first = mixed_games()[0].ids.clone();
    let second = mixed_games()[1].ids.clone();
    ui.act(|app| {
        let presence = AssetPresence {
            cover: true,
            background: false,
            icon_banner: true,
            screenshots: false,
        };
        mark(app, &first, PresenceCell::Known(presence));
        mark(app, &second, PresenceCell::Failed);
    });
    let rows = ui.rows();
    let banana = &rows[0].marks;
    assert_eq!(
        (
            banana.cover,
            banana.background,
            banana.icon_banner,
            banana.screenshots
        ),
        (1, 2, 1, 2)
    );
    let apple = &rows[1].marks;
    assert_eq!(
        (
            apple.cover,
            apple.background,
            apple.icon_banner,
            apple.screenshots
        ),
        (3, 3, 3, 3)
    );
}

fn mark(app: &mut App, ids: &GameIds, cell: PresenceCell) {
    let generation = app.console().refresh_generation();
    let result = match cell {
        PresenceCell::Known(presence) => Ok(presence),
        PresenceCell::Failed => Err(ConsoleError::TooManyConnections),
        PresenceCell::Unknown => return,
    };
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Presence {
        generation,
        ids: ids.clone(),
        result,
    }));
}

#[test]
fn clicking_a_header_sorts_and_a_row_selects() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    assert_eq!(ui.row_titles(), ["banana", "Apple", "cherry"]);
    assert_eq!(ui.console().get_sort_column(), -1);

    ui.click("GameTable::title-header");
    assert_eq!(
        harness.app().console().table.sort(),
        Some((GameColumn::Title, true))
    );
    assert_eq!(ui.row_titles(), ["Apple", "banana", "cherry"]);
    assert_eq!(ui.console().get_sort_column(), 0);
    assert!(ui.console().get_sort_ascending());

    ui.click("GameTable::title-header");
    assert_eq!(ui.row_titles(), ["cherry", "banana", "Apple"]);
    assert!(!ui.console().get_sort_ascending());

    ui.click("GameTable::disc-header");
    assert_eq!(ui.row_titles(), ["cherry", "banana", "Apple"]);
    assert_eq!(ui.console().get_sort_column(), 4);

    ui.click("GameTable::assets-header");
    assert_eq!(ui.console().get_sort_column(), 4, "Assets does not sort");

    assert_eq!(ui.console().get_actions_reason(), SELECT_A_GAME_REASON);
    ui.click_label_in("Apple", "ConsolePage::game-table");
    let game = harness
        .app()
        .current_game()
        .cloned()
        .expect("a game is selected");
    assert_eq!(game.title, "Apple");
    assert_eq!(game.ids.database_id.to_string(), "00000012");
    assert_eq!(ui.console().get_selected_row(), 2);
    assert!(ui.rows()[2].selected);
    assert_eq!(ui.console().get_actions_reason(), "");
    assert!(ui.enabled("ConsolePage::open-in-editor-button"));
    assert!(ui.enabled("ConsolePage::upload-button"));
}

#[test]
fn a_render_that_changes_the_row_count_keeps_a_click_on_a_row() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    let row = ui.labelled_in("banana", "ConsolePage::game-table");
    let (position, size) = (row.absolute_position(), row.size());
    let centre = slint::LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    let window = ui.view.window().window();
    window.dispatch_event(WindowEvent::PointerMoved { position: centre });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: centre,
        button: PointerEventButton::Left,
    });
    // The first row stays, the two after it go.
    ui.act(|app| app.set_game_filter("an", ""));
    assert_eq!(ui.row_titles(), ["banana"]);
    window.dispatch_event(WindowEvent::PointerReleased {
        position: centre,
        button: PointerEventButton::Left,
    });

    assert_eq!(ui.console().get_selected_row(), 0, "the row was selected");
}

#[test]
fn filters_from_the_two_boxes_are_anded() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));

    ui.click("ConsolePage::filter-title-id-field");
    ui.ui.type_text(ui.view.window().window(), "4D53");
    assert_eq!(ui.row_titles(), ["banana", "Apple"]);

    ui.click("ConsolePage::filter-title-field");
    ui.ui.type_text(ui.view.window().window(), "app");
    assert_eq!(ui.row_titles(), ["Apple"]);
    assert_eq!(ui.console().get_filter_title(), "app");
    assert_eq!(ui.console().get_filter_title_id(), "4D53");
}

#[test]
fn tab_reaches_the_table_and_arrows_select() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, mixed_games()));
    assert!(ui.labelled("Games").accessible_label().is_some());
    assert!(
        ElementHandle::find_by_element_id(ui.view.window(), "GameTable::focus-outline")
            .next()
            .is_none()
    );

    ui.click("ConsolePage::filter-title-id-field");
    ui.press(Key::Tab);
    ui.element("GameTable::focus-outline");
    assert_eq!(ui.console().get_selected_row(), -1);

    ui.press(Key::DownArrow);
    assert_eq!(
        ui.console().get_selected_row(),
        0,
        "the first arrow selects the first row"
    );
    assert_eq!(
        harness
            .app()
            .current_game()
            .map(|game| game.title.clone())
            .as_deref(),
        Some("banana")
    );
    ui.press(Key::DownArrow);
    assert_eq!(ui.console().get_selected_row(), 1);
}

#[test]
fn moving_the_selection_in_an_empty_table_does_nothing() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let app = &mut *harness.app();
    app.move_game_selection(1);
    app.select_last_game();
    app.select_first_game();
    assert!(app.console().table.selected().is_none());
    assert!(app.current_game().is_none());
}

#[test]
fn online_images_are_converted_once_per_result() {
    let mut cache = OnlineImageCache::default();
    let key = |generation, index, image| ResultImageKey {
        generation,
        index,
        image,
    };
    let converted = Cell::new(0);
    let convert_once = || {
        converted.set(converted.get() + 1);
        Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::new(1, 1))
    };
    let panic_on_second = || -> Image { panic!("the image was converted again") };

    let first = cache.image(key(1, 0, ResultImage::Thumbnail), convert_once);
    let again = cache.image(key(1, 0, ResultImage::Thumbnail), panic_on_second);
    assert_eq!(first, again);
    cache.image(key(1, 0, ResultImage::Full), convert_once);
    cache.image(key(1, 1, ResultImage::Thumbnail), convert_once);
    cache.image(key(1, 0, ResultImage::Full), panic_on_second);
    assert_eq!(converted.get(), 3);

    cache.image(key(2, 0, ResultImage::Thumbnail), convert_once);
    assert_eq!(converted.get(), 4, "a new search converts its own images");
}

#[test]
fn a_render_during_a_press_on_a_result_keeps_the_click() {
    let server = online_server();
    let mut harness = online_harness(&server, ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.edit_search_text(RESULTS_TITLE);
        app.search_online();
    });
    harness.run_jobs();
    ui.render();
    let first = ui.online().get_tiles().row_data(0).expect("a result").label;
    let tile = ui.labelled_in(&first, "OnlinePage::result-grid");
    let (position, size) = (tile.absolute_position(), tile.size());
    let centre = slint::LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    let window = ui.view.window().window();
    window.dispatch_event(WindowEvent::PointerMoved { position: centre });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: centre,
        button: PointerEventButton::Left,
    });
    // What the app does after any job outcome, such as another result's thumbnail arriving.
    ui.render();
    window.dispatch_event(WindowEvent::PointerReleased {
        position: centre,
        button: PointerEventButton::Left,
    });

    assert_eq!(ui.online().get_selected(), 0, "the result was selected");
}

#[test]
fn typing_does_not_reconvert_result_images() {
    let server = online_server();
    let mut harness = online_harness(&server, ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.edit_search_text(RESULTS_TITLE);
        app.search_online();
    });
    harness.run_jobs();
    ui.render();
    let first = ui.online().get_tiles().row_data(0).expect("a result").label;
    ui.click_label_in(&first, "OnlinePage::result-grid");
    harness.run_jobs();
    ui.render();
    let thumbnail = ui.online().get_tiles().row_data(0).expect("a result").image;
    let full = ui.online().get_selected_image();

    ui.click("TextField::input");
    ui.ui.type_text(ui.view.window().window(), "Z");

    assert_eq!(
        ui.online().get_tiles().row_data(0).expect("a result").image,
        thumbnail
    );
    assert_eq!(ui.online().get_selected_image(), full);
}

#[test]
fn table_keyboard_moves_selection() {
    let games: Vec<Game> = (0..MANY_GAMES)
        .map(|index| {
            let number = i64::try_from(index).expect("a small number");
            game(
                &format!("Game {index:02}"),
                &format!("4D53{index:04X}"),
                number,
                1,
            )
        })
        .collect();
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| show_games(app, games));

    ui.click_label_in("Game 01", "ConsolePage::game-table");
    assert_eq!(ui.console().get_selected_row(), 1);
    ui.press(Key::DownArrow);
    assert_eq!(ui.console().get_selected_row(), 2);
    ui.press(Key::UpArrow);
    ui.press(Key::UpArrow);
    assert_eq!(ui.console().get_selected_row(), 0);
    ui.press(Key::UpArrow);
    assert_eq!(ui.console().get_selected_row(), 0, "stays on the first row");

    ui.press(Key::End);
    let last = i32::try_from(MANY_GAMES - 1).expect("a small number");
    assert_eq!(ui.console().get_selected_row(), last);
    assert_eq!(
        harness
            .app()
            .current_game()
            .map(|game| game.title.clone())
            .as_deref(),
        Some("Game 59")
    );
    ui.press(Key::DownArrow);
    assert_eq!(
        ui.console().get_selected_row(),
        last,
        "stays on the last row"
    );
    let last_row = ui.labelled("Game 59");
    let table = ui.element("ConsolePage::game-table");
    let row_bottom = last_row.absolute_position().y + last_row.size().height;
    let table_bottom = table.absolute_position().y + table.size().height;
    assert!(
        row_bottom <= table_bottom + 0.5,
        "End scrolls the last row into view"
    );

    ui.press(Key::Home);
    assert_eq!(ui.console().get_selected_row(), 0);
    let first_row = ui.labelled("Game 00");
    assert!(first_row.absolute_position().y >= table.absolute_position().y);
}

#[test]
fn console_menus_open_above_their_buttons_and_act() {
    let console = typical_console();
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    harness.app().show_page(Page::Console);
    let ui = Ui::start(&mut harness);
    ui.click("ConsolePage::refresh-button");
    harness.run_jobs();
    ui.render();
    let first = ui.rows()[0].title.to_string();
    ui.click_label_in(&first, "ConsolePage::game-table");

    for (button, menu, menu_id) in [
        (
            "ConsolePage::download-button",
            Menu::Download,
            "MenuOverlay::download-menu",
        ),
        (
            "ConsolePage::upload-button",
            Menu::Upload,
            "MenuOverlay::upload-menu",
        ),
        (
            "ConsolePage::remove-button",
            Menu::Remove,
            "MenuOverlay::remove-menu",
        ),
    ] {
        ui.click(button);
        assert_eq!(harness.app().open_menu(), Some(menu));
        let button = ui.element(button);
        let list = ui.element(menu_id);
        assert_eq!(list.absolute_position().x, button.absolute_position().x);
        assert!(list.absolute_position().y + list.size().height <= button.absolute_position().y);
        ui.act(|app| app.toggle_menu(None));
        assert_eq!(
            (
                ui.console().get_menu_anchor_x(),
                ui.console().get_menu_anchor_y()
            ),
            (0.0, 0.0),
            "closing the menu resets its anchor"
        );
    }

    ui.click("ConsolePage::download-button");
    let queued = harness.queued_jobs();
    ui.click_label_in("Cover", "MenuOverlay::download-menu");
    assert_eq!(harness.app().open_menu(), None);
    assert_eq!(harness.queued_jobs(), queued + 1, "Download Cover started");
}

#[test]
fn long_and_non_latin_titles_render_without_panic() {
    let long_title: String = "Long title "
        .repeat(LONG_TITLE_LENGTH)
        .chars()
        .take(LONG_TITLE_LENGTH)
        .collect();
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        show_games(
            app,
            vec![
                game(&long_title, "4D5307E6", 1, 1),
                game("ヘイロー3", "4D5307E7", 2, 1),
            ],
        );
        app.select_game(1);
    });
    assert_eq!(
        ui.row_titles(),
        [long_title.clone(), "ヘイロー3".to_owned()]
    );
    assert!(
        ui.view
            .window()
            .global::<ShellModel>()
            .get_chip_text()
            .starts_with("ヘイロー3")
    );
    let snapshot = ui.snapshot();
    // Not compared as a picture: the Japanese glyphs come from the system's fonts.
    assert_eq!(
        (snapshot.width(), snapshot.height()),
        HeadlessUi::WINDOW_SIZE
    );
    ui.act(|app| app.select_game(0));
    ui.snapshot();
}
