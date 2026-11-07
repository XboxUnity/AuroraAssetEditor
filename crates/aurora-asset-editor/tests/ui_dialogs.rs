use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use aurora_asset_editor::app::{App, CurrentGame, KindChoice, Page};
use aurora_asset_editor::dialogs::Modal;
use aurora_asset_editor::jobs::{ConsoleOutcome, JobOutcome};
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::MainWindowView;
use aurora_console::{ConsoleError, FtpSettings, Game, GameIds, GameListing, HexId};
use aurora_ui::{DialogKind, DialogModel, MainWindow};
use editor_test_support::{AppHarness, HeadlessUi};
use i_slint_backend_testing::ElementHandle;
use image::{Rgba, RgbaImage};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};
use tempfile::TempDir;
use test_servers::{FakeConsole, FakeConsoleConfig};

const HALO_TITLE_ID: &str = "4D5307E6";
/// Set to a folder to keep the rendered dialogs as PNG files for a look.
const SNAPSHOT_DIR_VARIABLE: &str = "AURORA_UI_SNAPSHOT_DIR";
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

fn png(folder: &Path, name: &str) -> PathBuf {
    let path = folder.join(name);
    RgbaImage::from_pixel(64, 64, Rgba([200, 30, 30, 255]))
        .save(&path)
        .expect("the test image can be written");
    path
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

fn assert_close(actual: [u8; 3], expected: [u8; 3], tolerance: i16, what: &str) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| (i16::from(*actual) - i16::from(expected)).abs() <= tolerance);
    assert!(close, "{what}: {actual:?} is not close to {expected:?}");
}

/// One way to reach each dialog kind, with the title the "UI copy" gives it.
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
            keep_snapshot(&with_dialog, &format!("dialogs-{theme_name}-{}", flow.name));

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
