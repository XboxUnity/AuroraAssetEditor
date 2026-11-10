use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use aurora_asset::{AssetSlot, ScreenshotNumber};
use aurora_asset_editor::app::{App, CurrentGame, KindChoice, Page};
use aurora_asset_editor::dialogs::{BulkForm, Modal};
use aurora_asset_editor::editor::SlotPreview;
use aurora_asset_editor::jobs::{ConsoleOutcome, JobOutcome};
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::MainWindowView;
use aurora_console::{ConsoleError, FtpSettings, Game, GameIds, GameListing, HexId};
use aurora_online::CoverSource;
use aurora_ui::{DialogKind, DialogModel, MainWindow};
use editor_test_support::{AppHarness, HeadlessUi, PreviewJobs};
use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use image::{Rgba, RgbaImage};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, SharedString, VecModel};
use tempfile::TempDir;
use test_servers::{FakeConsole, FakeConsoleConfig};

const HALO_TITLE_ID: &str = "4D5307E6";
const THEMES: [(ThemeChoice, &str); 2] =
    [(ThemeChoice::Dark, "dark"), (ThemeChoice::Light, "light")];
/// A window corner, far from any card.
const BACKDROP_PROBE: (u32, u32) = (4, 4);
/// The backdrop is black at 50 %; rounding moves each channel by at most this much.
const BACKDROP_TOLERANCE: i16 = 2;
/// Inside a button's left padding, clear of its rounded corners and its focus outline.
const BUTTON_PROBE_INSET: f32 = 3.0;
const DANGER: [u8; 3] = [0xdc, 0x26, 0x26];
const ACCENT: [u8; 3] = [0x3b, 0x82, 0xf6];
const COLOUR_TOLERANCE: i16 = 3;
const LONG_MESSAGE_FILES: usize = 40;
const DIALOG_TEXT_MAX_HEIGHT: f32 = 320.0;
const PASSWORD: &str = "secret-word";
/// IP Address, Port, Username, Password, Test Connection, Save, Cancel.
const CONNECTION_FOCUS_STOPS: usize = 7;

struct Ui {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Ui {
    fn start(harness: &mut AppHarness) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window, PreviewJobs::new().runner());
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

    fn dialog(&self) -> DialogModel<'_> {
        self.view.window().global::<DialogModel>()
    }

    fn fields(&self) -> Vec<String> {
        self.dialog()
            .get_fields()
            .iter()
            .map(|field| field.to_string())
            .collect()
    }

    fn element(&self, id: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(self.view.window(), id)
            .next()
            .unwrap_or_else(|| panic!("the window shows {id}"))
    }

    fn labelled(&self, label: &str) -> Option<ElementHandle> {
        ElementHandle::find_by_accessible_label(self.view.window(), label).next()
    }

    fn click(&self, id: &str) {
        self.element(id).mock_single_click(PointerEventButton::Left);
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

    fn type_text(&self, text: &str) {
        self.ui.type_text(self.view.window().window(), text);
    }

    /// The colour inside the left padding of the button `id`.
    fn button_colour(&self, id: &str) -> [u8; 3] {
        let button = self.element(id);
        let (position, size) = (button.absolute_position(), button.size());
        let snapshot = self.snapshot();
        let pixel = snapshot.get_pixel(
            (position.x + BUTTON_PROBE_INSET) as u32,
            (position.y + size.height / 2.0) as u32,
        );
        [pixel[0], pixel[1], pixel[2]]
    }

    /// The colour of the button `id`'s left edge, where its focus outline is drawn.
    fn button_edge_colour(&self, id: &str) -> [u8; 3] {
        let button = self.element(id);
        let (position, size) = (button.absolute_position(), button.size());
        let snapshot = self.snapshot();
        let pixel = snapshot.get_pixel(position.x as u32, (position.y + size.height / 2.0) as u32);
        [pixel[0], pixel[1], pixel[2]]
    }
}

fn halo_3() -> CurrentGame {
    CurrentGame {
        title: "Halo 3".to_owned(),
        ids: GameIds {
            title_id: HexId::parse(HALO_TITLE_ID).expect("a valid TitleID"),
            database_id: HexId::from_database_value(18),
        },
    }
}

fn game(title: &str, title_id: &str, database_id: i64) -> Game {
    Game {
        title_name: title.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(title_id).expect("a valid TitleID"),
            database_id: HexId::from_database_value(database_id),
        },
        media_id: HexId::parse("1A2B3C4D").expect("a valid MediaID"),
        disc: 1,
    }
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

/// Answers the refresh an ID dialog started with `games`, as a console would.
fn answer_listing(app: &mut App, games: Vec<Game>) {
    let generation = app.console().refresh_generation();
    app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Refreshed {
        generation,
        result: Ok(GameListing {
            games,
            cleanup_error: None,
        }),
    }));
}

fn png(folder: &Path, name: &str) -> PathBuf {
    let path = folder.join(name);
    RgbaImage::from_pixel(64, 64, Rgba([200, 30, 30, 255]))
        .save(&path)
        .expect("the test image can be written");
    path
}

fn assert_close(actual: [u8; 3], expected: [u8; 3], tolerance: i16, what: &str) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| (i16::from(*actual) - i16::from(expected)).abs() <= tolerance);
    assert!(close, "{what}: {actual:?} is not close to {expected:?}");
}

/// One way to reach each dialog kind, with the title it shows.
struct DialogFlow {
    name: &'static str,
    kind: DialogKind,
    title: &'static str,
    open: fn(&mut AppHarness, &Ui),
}

const FLOWS: [DialogFlow; 7] = [
    DialogFlow {
        name: "message",
        kind: DialogKind::Message,
        title: "Could not load files",
        open: |harness, ui| {
            ui.act(|app| app.open_paths(vec![PathBuf::from("missing-file.asset")]));
            harness.run_jobs();
            ui.render();
        },
    },
    DialogFlow {
        name: "choice",
        kind: DialogKind::Choice,
        title: "Icon or banner?",
        open: |_, ui| {
            let folder = TempDir::new().expect("a temporary folder");
            let image = png(folder.path(), "art.png");
            ui.act(|app| {
                app.show_page(Page::IconBanner);
                app.files_dropped(vec![image]);
            });
        },
    },
    DialogFlow {
        name: "title-id",
        kind: DialogKind::TitleId,
        title: "Save all assets",
        open: |_, ui| {
            ui.act(|app| {
                app.set_current_game(Some(halo_3()));
                app.save_all_locally();
            });
        },
    },
    DialogFlow {
        name: "upload-target",
        kind: DialogKind::UploadTarget,
        title: "Save all assets to console",
        open: |_, ui| ui.act(|app| app.save_all_to_console()),
    },
    DialogFlow {
        name: "confirm",
        kind: DialogKind::Confirm,
        title: "Remove from console?",
        open: |_, ui| {
            ui.act(|app| {
                app.set_current_game(Some(halo_3()));
                app.remove_from_console(KindChoice::All);
            });
        },
    },
    DialogFlow {
        name: "connection",
        kind: DialogKind::Connection,
        title: "Console connection",
        open: |_, ui| ui.act(|app| app.edit_connection()),
    },
    DialogFlow {
        name: "bulk",
        kind: DialogKind::Bulk,
        title: "Bulk actions",
        open: |_, ui| {
            ui.act(|app| {
                show_games(app, vec![game("Halo 3", HALO_TITLE_ID, 18)]);
                app.open_bulk_actions();
            });
        },
    },
];

fn open_flow(flow: &DialogFlow, theme: ThemeChoice) -> (AppHarness, Ui) {
    let mut harness = console_harness(theme);
    let ui = Ui::start(&mut harness);
    (flow.open)(&mut harness, &ui);
    (harness, ui)
}

#[test]
fn every_dialog_has_a_title() {
    for flow in &FLOWS {
        let (_harness, ui) = open_flow(flow, ThemeChoice::Dark);
        assert_eq!(ui.dialog().get_kind(), flow.kind, "{}", flow.name);
        assert_eq!(ui.dialog().get_title(), flow.title, "{}", flow.name);
        assert!(
            ui.labelled(flow.title).is_some(),
            "the {} dialog draws its title",
            flow.name
        );
        assert!(
            !ui.dialog().get_title().starts_with("ERROR"),
            "{} has no ERROR prefix",
            flow.name
        );
    }
}

#[test]
fn dialog_keys_accept_and_cancel() {
    let confirm = &FLOWS[4];
    let (harness, ui) = open_flow(confirm, ThemeChoice::Dark);
    ui.press(Key::Escape);
    assert_eq!(ui.app.borrow().modal(), None, "Escape cancels");
    assert_eq!(harness.queued_jobs(), 0, "a cancelled removal does nothing");
    assert_eq!(ui.dialog().get_kind(), DialogKind::None);

    ui.act(|app| app.remove_from_console(KindChoice::All));
    ui.press(Key::Return);
    assert_eq!(ui.app.borrow().modal(), None, "Return accepts");
    assert_eq!(harness.queued_jobs(), 1, "the accepted removal is queued");

    let title_id = &FLOWS[2];
    let (_harness, ui) = open_flow(title_id, ThemeChoice::Dark);
    ui.type_text("4D5");
    assert!(!ui.dialog().get_accept_enabled());
    ui.press(Key::Return);
    assert!(
        matches!(ui.app.borrow().modal(), Some(Modal::TitleId { .. })),
        "Return does nothing while OK is disabled"
    );
    ui.press(Key::Escape);
    assert_eq!(ui.app.borrow().modal(), None, "Escape cancels from the box");

    let choice = &FLOWS[1];
    let (harness, ui) = open_flow(choice, ThemeChoice::Dark);
    ui.press(Key::Return);
    assert_eq!(ui.app.borrow().modal(), None, "Return cancels a choice");
    assert_eq!(
        harness.queued_jobs(),
        0,
        "the cancelled file is not imported"
    );
}

#[test]
fn tab_reaches_dialog_buttons_with_a_focus_outline() {
    let confirm = &FLOWS[4];
    let (harness, ui) = open_flow(confirm, ThemeChoice::Dark);
    let unfocused = ui.button_edge_colour("ConfirmDialog::cancel-button");

    ui.press(Key::Tab);
    ui.press(Key::Tab);
    let focused = ui.button_edge_colour("ConfirmDialog::cancel-button");
    assert_ne!(unfocused, focused, "the focused button gets an outline");
    assert_close(focused, ACCENT, COLOUR_TOLERANCE, "focus outline");

    ui.press(Key::Return);
    assert_eq!(
        ui.app.borrow().modal(),
        None,
        "Return clicks the focused Cancel"
    );
    assert_eq!(harness.queued_jobs(), 0, "Cancel, not Remove, was clicked");
}

#[test]
fn title_id_prompt_selects_all_on_show() {
    let (_harness, ui) = open_flow(&FLOWS[2], ThemeChoice::Dark);
    assert_eq!(ui.fields(), [HALO_TITLE_ID]);

    ui.type_text("4D5307E7");

    assert_eq!(ui.fields(), ["4D5307E7"]);
    match ui.app.borrow().modal() {
        Some(Modal::TitleId { form, .. }) => assert_eq!(form.title_id, "4D5307E7"),
        other => panic!("expected the TitleID prompt, got {other:?}"),
    }
    assert_eq!(
        ui.element("TextField::input").accessible_value(),
        Some("4D5307E7".into())
    );
}

#[test]
fn ok_disabled_until_ids_valid() {
    let (_harness, ui) = open_flow(&FLOWS[3], ThemeChoice::Dark);
    let ok = "UploadTargetDialog::ok-button";
    assert_eq!(ui.fields(), ["", ""]);
    assert!(!ui.enabled(ok));

    ui.type_text("4d53-07e6x");
    assert_eq!(
        ui.fields(),
        ["4d5307e6", ""],
        "the TitleID box takes hex only"
    );
    assert!(!ui.enabled(ok), "the DatabaseID is still empty");

    let database_id = ElementHandle::find_by_accessible_label(ui.view.window(), "DatabaseID:")
        .find(|element| element.accessible_value().is_some())
        .expect("the DatabaseID box is labelled");
    database_id.mock_single_click(PointerEventButton::Left);
    ui.type_text("1234567");
    assert!(!ui.enabled(ok), "seven digits are not a DatabaseID");
    ui.type_text("8");
    assert_eq!(ui.fields(), ["4d5307e6", "12345678"]);
    assert!(ui.enabled(ok));

    // OK waits for the console, which lists its games for the dialog's picker.
    ui.act(|app| answer_listing(app, Vec::new()));
    ui.press(Key::Return);
    assert_eq!(ui.app.borrow().modal(), None, "Return sends once OK works");
}

#[test]
fn connection_dialog_shows_test_status() {
    let typical = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/console/content-db/typical.db"),
    )
    .expect("the sample database is readable");
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical);
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    let ui = Ui::start(&mut harness);
    ui.act(|app| app.edit_connection());
    assert_eq!(ui.dialog().get_test_status(), "");

    ui.click("ConnectionDialog::test-button");
    harness.run_jobs();
    ui.render();
    let status = ui.dialog().get_test_status().to_string();
    assert!(
        status.starts_with("Connection to Aurora revision "),
        "a working test says so: {status}"
    );
    assert!(!ui.dialog().get_test_status_is_error());
    assert!(ui.labelled(&status).is_some(), "the dialog shows {status}");

    ui.click("ConnectionDialog::test-button");
    ui.act(|app| {
        app.on_job_outcome(JobOutcome::Console(ConsoleOutcome::Tested(Err(
            ConsoleError::LoginRejected { code: 530 },
        ))));
    });
    let status = ui.dialog().get_test_status().to_string();
    assert!(
        status.starts_with("Error: "),
        "a failed test says so: {status}"
    );
    assert!(ui.dialog().get_test_status_is_error());
    assert!(ui.labelled(&status).is_some(), "the dialog shows {status}");
}

#[test]
fn dialogs_render_in_both_themes() {
    for (theme, theme_name) in THEMES {
        let mut snapshots = Vec::new();
        for flow in &FLOWS {
            let mut harness = console_harness(theme);
            let ui = Ui::start(&mut harness);
            let without_dialog = ui.snapshot();
            (flow.open)(&mut harness, &ui);
            let with_dialog = ui.snapshot();

            let (x, y) = BACKDROP_PROBE;
            let below = without_dialog.get_pixel(x, y);
            let dimmed = with_dialog.get_pixel(x, y);
            let halved = [below[0] / 2, below[1] / 2, below[2] / 2];
            assert_close(
                [dimmed[0], dimmed[1], dimmed[2]],
                halved,
                BACKDROP_TOLERANCE,
                "the backdrop dims the window by half",
            );
            if flow.kind == DialogKind::Confirm {
                assert_close(
                    ui.button_colour("ConfirmDialog::confirm-button"),
                    DANGER,
                    COLOUR_TOLERANCE,
                    "Remove is drawn as dangerous",
                );
            }
            snapshots.push((flow.name, with_dialog));
        }
        for (index, (name, image)) in snapshots.iter().enumerate() {
            for (other_name, other) in &snapshots[index + 1..] {
                assert_ne!(image, other, "{theme_name}: {name} and {other_name} differ");
            }
        }
    }
}

fn text_input(ui: &Ui, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui.view.window(), label)
        .find(|element| element.accessible_role() == Some(AccessibleRole::TextInput))
        .unwrap_or_else(|| panic!("the {label} box is on screen"))
}

fn is_empty(app: &App, slot: AssetSlot) -> bool {
    let document = match slot {
        AssetSlot::Icon | AssetSlot::Banner => &app.pages().icon_banner,
        _ => &app.pages().screenshots,
    };
    *document.preview(slot) == SlotPreview::Empty
}

fn shot(position: u8) -> AssetSlot {
    AssetSlot::Screenshot(ScreenshotNumber::new(position).expect("a screenshot number"))
}

fn two_games() -> Vec<Game> {
    vec![
        game("Halo 3", HALO_TITLE_ID, 18),
        game("Fable", "4D5307E7", 19),
    ]
}

#[test]
fn focus_stays_inside_an_open_dialog() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.show_page(Page::Console);
        show_games(app, two_games());
        app.edit_connection();
    });

    for _ in 0..CONNECTION_FOCUS_STOPS {
        ui.press(Key::Tab);
    }
    ui.type_text("9");
    assert_eq!(
        ui.fields()[0],
        "9",
        "Tab wrapped around to the IP Address box"
    );

    for _ in 0..3 * CONNECTION_FOCUS_STOPS {
        ui.press(Key::Tab);
        ui.press(Key::DownArrow);
    }
    assert!(
        ui.app.borrow().current_game().is_none(),
        "the game table never got the keys"
    );

    // Three whole rounds end on the IP Address box again; Shift+Tab from there wraps to the end.
    ui.press(Key::Backtab);
    let cancel = ui.button_edge_colour("ConnectionDialog::cancel-button");
    assert_close(
        cancel,
        ACCENT,
        COLOUR_TOLERANCE,
        "Shift+Tab wraps to Cancel",
    );
    ui.press(Key::Return);
    assert_eq!(ui.app.borrow().modal(), None, "Return clicked Cancel");
}

const BULK_TEXT_CASES: [(CoverSource, bool, usize, &str); 8] = [
    (
        CoverSource::XboxUnity,
        false,
        1,
        "Downloads a random cover from XboxUnity for the listed game if it has no cover on the console, and saves it there. Covers are not resized.",
    ),
    (
        CoverSource::XboxUnity,
        true,
        1,
        "Downloads a random cover from XboxUnity for the listed game and saves it on the console, replacing any cover already there. Covers are not resized.",
    ),
    (
        CoverSource::InternetArchive,
        false,
        1,
        "Downloads a random cover from Internet Archive for the listed game if it has no cover on the console, and saves it there. Covers are not resized.",
    ),
    (
        CoverSource::InternetArchive,
        true,
        1,
        "Downloads a random cover from Internet Archive for the listed game and saves it on the console, replacing any cover already there. Covers are not resized.",
    ),
    (
        CoverSource::XboxUnity,
        false,
        2,
        "Downloads a random cover from XboxUnity for each of the 2 listed games that has no cover on the console, and saves it there. Covers are not resized.",
    ),
    (
        CoverSource::XboxUnity,
        true,
        2,
        "Downloads a random cover from XboxUnity for each of the 2 listed games and saves it on the console, replacing any cover already there. Covers are not resized.",
    ),
    (
        CoverSource::InternetArchive,
        false,
        2,
        "Downloads a random cover from Internet Archive for each of the 2 listed games that has no cover on the console, and saves it there. Covers are not resized.",
    ),
    (
        CoverSource::InternetArchive,
        true,
        2,
        "Downloads a random cover from Internet Archive for each of the 2 listed games and saves it on the console, replacing any cover already there. Covers are not resized.",
    ),
];

#[test]
fn bulk_text_follows_the_source_the_box_and_the_game_count() {
    for (source, replace, game_count, expected) in BULK_TEXT_CASES {
        let mut harness = console_harness(ThemeChoice::Dark);
        let ui = Ui::start(&mut harness);
        ui.act(|app| {
            let mut games = two_games();
            games.truncate(game_count);
            show_games(app, games);
            app.open_bulk_actions();
            app.set_bulk_source(source);
            app.set_bulk_replace(replace);
        });
        assert_eq!(ui.dialog().get_text(), expected);
    }
}

#[test]
fn bulk_text_updates_while_the_dialog_is_open() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        show_games(app, two_games());
        app.open_bulk_actions();
    });
    assert!(ui.dialog().get_text().contains("from XboxUnity for each"));
    assert!(ui.dialog().get_text().contains("that has no cover"));

    ui.act(|app| app.set_bulk_source(CoverSource::InternetArchive));
    assert!(ui.dialog().get_text().contains("from Internet Archive"));

    ui.act(|app| app.set_bulk_replace(true));
    assert!(ui.dialog().get_text().contains("replacing any cover"));
}

#[test]
fn long_message_caps_its_height_and_scrolls() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    let missing = (0..LONG_MESSAGE_FILES)
        .map(|number| PathBuf::from(format!("missing-{number}.asset")))
        .collect();
    ui.act(|app| app.open_paths(missing));
    harness.run_jobs();
    ui.render();

    let scroller = ui.element("DialogText::scroller");
    assert_eq!(scroller.size().height, DIALOG_TEXT_MAX_HEIGHT);
    let thumb_top = ui.element("DialogText::scroll-thumb").absolute_position().y;

    let (position, size) = (scroller.absolute_position(), scroller.size());
    ui.view
        .window()
        .window()
        .dispatch_event(WindowEvent::PointerScrolled {
            position: LogicalPosition::new(
                position.x + size.width / 2.0,
                position.y + size.height / 2.0,
            ),
            delta_x: 0.0,
            delta_y: -100.0,
        });
    ui.render();
    let scrolled_top = ui.element("DialogText::scroll-thumb").absolute_position().y;
    assert!(
        scrolled_top > thumb_top,
        "the wheel scrolls the text and the thumb follows: {thumb_top} -> {scrolled_top}"
    );

    let (_harness, short) = open_flow(&FLOWS[0], ThemeChoice::Dark);
    assert!(
        ElementHandle::find_by_element_id(short.view.window(), "DialogText::scroll-thumb")
            .next()
            .is_none(),
        "a short message draws no thumb"
    );
}

#[test]
fn password_box_shows_its_text_like_the_username_box() {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: "192.168.1.50".to_owned(),
        port: "21".to_owned(),
        username: PASSWORD.to_owned(),
        password: PASSWORD.to_owned(),
    });
    let ui = Ui::start(&mut harness);
    ui.act(|app| app.edit_connection());

    let password = text_input(&ui, "Password:");
    let username = text_input(&ui, "Username:");
    assert_eq!(password.accessible_value().unwrap_or_default(), PASSWORD);
    assert_eq!(username.accessible_value().unwrap_or_default(), PASSWORD);

    let snapshot = ui.snapshot();
    let crop = |element: &ElementHandle| {
        let (position, size) = (element.absolute_position(), element.size());
        image::imageops::crop_imm(
            &snapshot,
            position.x as u32,
            position.y as u32,
            size.width as u32,
            size.height as u32,
        )
        .to_image()
    };
    assert_eq!(
        crop(&username),
        crop(&password),
        "the same text is drawn the same way in both boxes"
    );
}

#[test]
fn choice_buttons_answer_with_their_choice() {
    let folder = TempDir::new().expect("a temporary folder");
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| {
        app.show_page(Page::IconBanner);
        app.files_dropped(vec![png(folder.path(), "icon.png")]);
    });
    ui.click("ChoiceDialog::first-choice-button");
    harness.run_jobs();
    assert!(
        !is_empty(&ui.app.borrow(), AssetSlot::Icon),
        "Icon took the file"
    );
    assert!(is_empty(&ui.app.borrow(), AssetSlot::Banner));

    ui.act(|app| app.files_dropped(vec![png(folder.path(), "banner.png")]));
    ui.click("ChoiceDialog::second-choice-button");
    harness.run_jobs();
    assert!(
        !is_empty(&ui.app.borrow(), AssetSlot::Banner),
        "Banner took the file"
    );

    ui.act(|app| app.show_page(Page::Screenshots));
    harness
        .dialogs()
        .push_files(vec![png(folder.path(), "first.png")]);
    ui.act(|app| app.replace_image(shot(1)));
    harness.run_jobs();
    ui.act(|app| app.files_dropped(vec![png(folder.path(), "second.png")]));
    assert_eq!(ui.dialog().get_kind(), DialogKind::Choice);
    ui.click("ChoiceDialog::second-choice-button");
    harness.run_jobs();
    assert!(
        !is_empty(&ui.app.borrow(), shot(2)),
        "Add to free slots put the file in slot 2"
    );
}

#[test]
fn a_render_while_typing_keeps_the_text_caret_and_focus() {
    let (_harness, ui) = open_flow(&FLOWS[5], ThemeChoice::Dark);
    ui.type_text("10.0.0.2");
    ui.press(Key::LeftArrow);
    ui.press(Key::LeftArrow);
    // What the app does after any job outcome, such as a connection test's status.
    ui.render();
    ui.type_text("9");

    let host = "10.0.09.2";
    assert_eq!(ui.fields()[0], host, "the digit went in at the caret");
    assert_eq!(
        text_input(&ui, "IP Address:")
            .accessible_value()
            .unwrap_or_default(),
        host
    );
}

#[test]
fn a_render_during_a_press_on_a_choice_keeps_the_click() {
    let (_harness, ui) = open_flow(&FLOWS[1], ThemeChoice::Dark);
    let button = ui.element("ChoiceDialog::first-choice-button");
    let (position, size) = (button.absolute_position(), button.size());
    let centre = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    let window = ui.view.window().window();
    window.dispatch_event(WindowEvent::PointerMoved { position: centre });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: centre,
        button: PointerEventButton::Left,
    });
    ui.render();
    window.dispatch_event(WindowEvent::PointerReleased {
        position: centre,
        button: PointerEventButton::Left,
    });

    assert_eq!(ui.app.borrow().modal(), None, "the choice was made");
}

#[test]
fn connection_boxes_edit_their_fields_and_save_writes_them() {
    let (harness, ui) = open_flow(&FLOWS[5], ThemeChoice::Dark);
    let typed = [
        ("IP Address:", "10.0.0.2"),
        ("Port:", "2121"),
        ("Username:", "aurora"),
        ("Password:", "hunter"),
    ];
    for (index, (_, text)) in typed.iter().enumerate() {
        if index > 0 {
            // Tab into the next box selects its text, so typing replaces it. On Apple Slint leaves
            // the caret where it was instead, so the box is emptied on both sides of it first.
            ui.press(Key::Tab);
            if cfg!(target_vendor = "apple") {
                let prefill = ui.fields()[index].chars().count();
                for _ in 0..prefill {
                    ui.press(Key::Backspace);
                    ui.press(Key::Delete);
                }
            }
        }
        ui.type_text(text);
    }
    match ui.app.borrow().modal() {
        Some(Modal::Connection { form }) => {
            assert_eq!(form.host, "10.0.0.2");
            assert_eq!(form.port, "2121");
            assert_eq!(form.username, "aurora");
            assert_eq!(form.password, "hunter");
        }
        other => panic!("expected the connection dialog, got {other:?}"),
    }
    assert_eq!(ui.fields(), typed.map(|(_, text)| text));
    for (label, text) in &typed[..3] {
        assert_eq!(
            text_input(&ui, label)
                .accessible_value()
                .unwrap_or_default(),
            *text
        );
    }

    ui.click("ConnectionDialog::save-button");
    assert_eq!(ui.app.borrow().modal(), None, "Save closes the dialog");
    let saved = std::fs::read_to_string(harness.config_dir().join("ftp.json"))
        .expect("Save writes ftp.json");
    assert!(
        saved.contains("10.0.0.2") && saved.contains("hunter"),
        "{saved}"
    );
}

#[test]
fn bulk_controls_reach_the_form_and_start_runs() {
    let (harness, ui) = open_flow(&FLOWS[6], ThemeChoice::Dark);
    ElementHandle::find_by_accessible_label(ui.view.window(), "Internet Archive")
        .last()
        .expect("the source switch is on screen")
        .mock_single_click(PointerEventButton::Left);
    ui.labelled("Replace existing covers")
        .expect("the check box is on screen")
        .mock_single_click(PointerEventButton::Left);
    match ui.app.borrow().modal() {
        Some(Modal::Bulk { form }) => {
            assert_eq!(form.source, CoverSource::InternetArchive);
            assert!(form.replace_existing);
        }
        other => panic!("expected the bulk dialog, got {other:?}"),
    }
    assert_eq!(ui.dialog().get_bulk_source_index(), 1);
    assert!(ui.dialog().get_bulk_replace());

    let queued = harness.queued_jobs();
    ui.click("BulkDialog::start-button");
    assert_eq!(ui.app.borrow().modal(), None, "Start closes the dialog");
    assert_eq!(harness.queued_jobs(), queued + 1, "Start queues the run");
}

fn bulk_form(ui: &Ui) -> BulkForm {
    match ui.app.borrow().modal() {
        Some(Modal::Bulk { form }) => form.clone(),
        other => panic!("expected the bulk dialog, got {other:?}"),
    }
}

#[test]
fn bulk_source_switch_works_from_the_keyboard() {
    let (harness, ui) = open_flow(&FLOWS[6], ThemeChoice::Dark);
    let queued = harness.queued_jobs();

    ui.press(Key::Tab);
    ui.press(Key::Space);
    assert_eq!(bulk_form(&ui).source, CoverSource::InternetArchive, "Space");
    assert_eq!(ui.dialog().get_bulk_source_index(), 1);
    ui.press(Key::RightArrow);
    assert_eq!(bulk_form(&ui).source, CoverSource::XboxUnity, "Right wraps");
    ui.press(Key::LeftArrow);
    assert_eq!(bulk_form(&ui).source, CoverSource::InternetArchive, "Left");

    ui.press(Key::Tab);
    ui.press(Key::Space);
    let form = bulk_form(&ui);
    assert!(form.replace_existing, "Tab moved on to the check box");
    assert_eq!(form.source, CoverSource::InternetArchive);

    ui.press(Key::Backtab);
    ui.press(Key::LeftArrow);
    assert_eq!(
        bulk_form(&ui).source,
        CoverSource::XboxUnity,
        "Shift+Tab came back to the switch"
    );
    ui.press(Key::Backtab);
    ui.press(Key::Return);
    assert_eq!(
        ui.app.borrow().modal(),
        None,
        "Shift+Tab from the switch wrapped to Cancel"
    );
    assert_eq!(
        harness.queued_jobs(),
        queued,
        "Cancel, not Start, was clicked"
    );
}

#[test]
fn escape_cancels_every_dialog() {
    for flow in [&FLOWS[0], &FLOWS[3], &FLOWS[5], &FLOWS[6]] {
        let (_harness, ui) = open_flow(flow, ThemeChoice::Dark);
        ui.press(Key::Escape);
        assert_eq!(ui.app.borrow().modal(), None, "Escape closes {}", flow.name);
    }
}

/// The frame's own guard, without an App behind the model.
#[test]
fn frame_return_accepts_only_while_accept_is_enabled() {
    let headless = HeadlessUi::new();
    let window = MainWindow::new().expect("the main window builds on the testing backend");
    headless.show(window.window());
    let accepted = Rc::new(Cell::new(0));
    let dialog = window.global::<DialogModel>();
    let counter = Rc::clone(&accepted);
    dialog.on_accept(move || counter.set(counter.get() + 1));
    dialog.set_fields(ModelRc::new(VecModel::from(vec![SharedString::from(
        "4D5",
    )])));
    dialog.set_accept_enabled(false);
    dialog.set_kind(DialogKind::TitleId);
    // The first render creates the dialog, which puts the focus in its box.
    headless.snapshot(window.window());
    let press = |key: Key| {
        let text = SharedString::from(key);
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };

    press(Key::Return);
    assert_eq!(
        accepted.get(),
        0,
        "Return is ignored while accept is disabled"
    );

    dialog.set_accept_enabled(true);
    press(Key::Return);
    assert_eq!(accepted.get(), 1, "Return accepts once it is enabled");
}

fn picker_titles(ui: &Ui) -> Vec<String> {
    ui.dialog()
        .get_picker_games()
        .iter()
        .map(|game| game.title.to_string())
        .collect()
}

fn picker_row(ui: &Ui, title: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui.view.window(), title)
        .find(|element| element.accessible_role() == Some(AccessibleRole::ListItem))
        .unwrap_or_else(|| panic!("the picker lists {title}"))
}

#[test]
fn the_game_picker_filters_as_typed_and_picks_by_arrows_return_and_click() {
    let mut harness = console_harness(ThemeChoice::Dark);
    let ui = Ui::start(&mut harness);
    ui.act(|app| app.save_all_to_console());
    assert!(ui.dialog().get_picker_shown());
    assert_eq!(ui.dialog().get_picker_note(), "Refreshing the game list…");

    ui.act(|app| answer_listing(app, two_games()));
    assert_eq!(ui.dialog().get_picker_note(), "");
    ui.click("GamePicker::filter-field");
    ui.type_text("4d53");
    assert_eq!(picker_titles(&ui), ["Halo 3", "Fable"]);
    assert_eq!(ui.dialog().get_picker_highlighted(), 0);

    ui.press(Key::DownArrow);
    assert_eq!(ui.dialog().get_picker_highlighted(), 1);
    ui.press(Key::Return);
    assert_eq!(ui.fields(), ["4D5307E7", "00000013"]);
    assert_eq!(ui.dialog().get_picker_highlighted(), -1);

    picker_row(&ui, "Halo 3").mock_single_click(PointerEventButton::Left);
    assert_eq!(ui.fields(), ["4D5307E6", "00000012"]);

    ui.click("GamePicker::filter-field");
    ui.press(Key::Return);
    assert_eq!(
        ui.app.borrow().modal(),
        None,
        "with nothing highlighted, Return is OK"
    );
}
