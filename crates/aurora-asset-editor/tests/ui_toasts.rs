use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use aurora_asset::{AssetKind, AssetSlot, Compression, RgbaImage};
use aurora_asset_editor::app::{App, CurrentGame, KindChoice, Page};
use aurora_asset_editor::status::{MAX_TOASTS, Severity, TOAST_DURATION};
use aurora_asset_editor::view::{APP_BUSY_RETRY, MainWindowView};
use aurora_console::{GameIds, HexId, asset_folder};
use aurora_ui::{MainWindow, ShellModel, ToastView};
use editor_test_support::{AppHarness, HeadlessUi, PreviewJobs, asset_file_with};
use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use image::Rgba;
use slint::platform::PointerEventButton;
use slint::{ComponentHandle, Model};
use test_servers::{FakeConsole, FakeConsoleConfig};

const TOAST_CARD: &str = "ToastStack::toast-card";
const CLIPBOARD_FAILURE: &str = "clipboard is gone";
const DARK_ERROR_TEXT: Rgba<u8> = Rgba([0xf8, 0x71, 0x71, 0xff]);
const COLOUR_TOLERANCE: u8 = 24;
const A_MOMENT: Duration = Duration::from_millis(1);

/// The app in a connected window, as `main` runs it.
struct Window {
    ui: HeadlessUi,
    view: MainWindowView,
    app: Rc<RefCell<App>>,
}

impl Window {
    fn start(harness: &mut AppHarness) -> Window {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = MainWindowView::new(window, PreviewJobs::new().runner());
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let window = Window { ui, view, app };
        window.render();
        window
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }

    fn act(&self, action: impl FnOnce(&mut App)) {
        action(&mut self.app.borrow_mut());
        self.render();
    }

    fn toasts(&self) -> Vec<ToastView> {
        self.view
            .window()
            .global::<ShellModel>()
            .get_toasts()
            .iter()
            .collect()
    }

    fn toast_texts(&self) -> Vec<String> {
        self.toasts()
            .iter()
            .map(|toast| toast.text.to_string())
            .collect()
    }

    fn cards(&self) -> Vec<ElementHandle> {
        ElementHandle::find_by_element_id(self.view.window(), TOAST_CARD).collect()
    }

    /// Whether the first toast's card has a pixel of about `colour`: its border's and text's colour.
    fn card_shows(&self, colour: Rgba<u8>) -> bool {
        let card = &self.cards()[0];
        let (position, size) = (card.absolute_position(), card.size());
        let snapshot = self.ui.snapshot(self.view.window().window());
        let area = image::imageops::crop_imm(
            &snapshot,
            position.x as u32,
            position.y as u32,
            size.width as u32,
            size.height as u32,
        )
        .to_image();
        area.pixels().any(|pixel| near(*pixel, colour))
    }
}

/// Anti-aliased edges and glyphs do not quite reach their colour.
fn near(pixel: Rgba<u8>, expected: Rgba<u8>) -> bool {
    pixel
        .0
        .iter()
        .zip(expected.0)
        .all(|(a, b)| a.abs_diff(b) <= COLOUR_TOLERANCE)
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

/// Copies the TitleID and the DatabaseID by turns, `count` times: each copy is one toast.
fn copy_ids(window: &Window, count: usize) {
    for copy in 0..count {
        window.act(|app| {
            if copy % 2 == 0 {
                app.copy_title_id();
            } else {
                app.copy_database_id();
            }
        });
    }
}

fn game_harness() -> AppHarness {
    let mut harness = AppHarness::new();
    harness.app().set_current_game(Some(halo()));
    harness
}

#[test]
fn an_upload_that_completes_shows_its_status_in_a_toast() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_dir(&asset_folder(&halo().ids));
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    let folder = tempfile::tempdir().expect("tests need a temp folder");
    let path = folder.path().join("cover.asset");
    let pixels = RgbaImage::from_pixel(8, 8, Rgba([10, 40, 90, 255]));
    let mut cover = asset_file_with(&[(AssetSlot::Cover, pixels, Compression::None)]);
    std::fs::write(&path, cover.to_bytes()).expect("the temp folder is writable");
    harness.app().open_paths(vec![path]);
    harness.run_jobs();
    harness.app().set_current_game(Some(halo()));
    let window = Window::start(&mut harness);
    assert!(window.toasts().is_empty(), "loading a file is not a toast");

    window.act(|app| app.upload_current(KindChoice::One(AssetKind::Cover)));
    harness.run_jobs();
    window.render();

    let status = harness.app().status().text.clone();
    assert_eq!(status, "Uploaded cover to the console");
    assert_eq!(
        window.toasts(),
        [ToastView {
            text: status.as_str().into(),
            error: false,
        }]
    );
    let cards = window.cards();
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0].accessible_label().as_deref(),
        Some(status.as_str()),
        "a screen reader reads the toast"
    );
    assert!(
        !window.card_shows(DARK_ERROR_TEXT),
        "only a failure is drawn in the error colour"
    );
}

#[test]
fn a_failure_only_the_status_line_shows_gets_a_red_toast() {
    let mut harness = game_harness();
    harness.clipboard().fail_with(CLIPBOARD_FAILURE);
    let window = Window::start(&mut harness);

    window.act(App::copy_title_id);

    let status = window.app.borrow().status().clone();
    assert_eq!(status.severity, Severity::Error);
    assert_eq!(
        window.toasts(),
        [ToastView {
            text: status.text.as_str().into(),
            error: true,
        }]
    );
    assert!(
        window.card_shows(DARK_ERROR_TEXT),
        "the card is drawn in the error colour"
    );
}

#[test]
fn a_failure_a_dialog_shows_gets_no_toast() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.refuse_new_connections();
    let mut harness = AppHarness::new();
    harness.with_console(&console);
    harness.app().set_current_game(Some(halo()));
    let window = Window::start(&mut harness);

    window.act(|app| app.download(KindChoice::All));
    harness.run_jobs();
    window.render();

    assert!(harness.app().modal().is_some(), "the dialog says it");
    assert_eq!(harness.app().status().severity, Severity::Error);
    assert!(window.toasts().is_empty());
}

#[test]
fn a_toast_is_gone_after_four_seconds() {
    let mut harness = game_harness();
    let window = Window::start(&mut harness);
    copy_ids(&window, 1);
    assert_eq!(window.toast_texts(), ["Copied TitleID 4D5307E6"]);

    mock_elapsed_time(TOAST_DURATION - A_MOMENT);
    assert_eq!(window.toasts().len(), 1, "still there just before its time");

    mock_elapsed_time(A_MOMENT);
    assert!(window.toasts().is_empty());
    assert_eq!(window.app.borrow().toasts().iter().count(), 0);
    assert_eq!(
        window.app.borrow().status().text,
        "Copied TitleID 4D5307E6",
        "the status line keeps the text"
    );
}

#[test]
fn each_toast_ends_four_seconds_after_it_showed_whatever_renders_since() {
    let mut harness = game_harness();
    let window = Window::start(&mut harness);
    let half = TOAST_DURATION / 2;
    window.act(App::copy_title_id);
    mock_elapsed_time(half);

    window.act(App::copy_database_id);
    window.render();
    assert_eq!(window.toasts().len(), 2);

    mock_elapsed_time(half);
    assert_eq!(
        window.toast_texts(),
        ["Copied DatabaseID 00000012"],
        "the first ends on its own time, not 4 s after the last render"
    );
    mock_elapsed_time(half);
    assert!(window.toasts().is_empty(), "the second ends 2 s later");
}

#[test]
fn a_toast_whose_time_ends_while_an_action_holds_the_app_ends_once_it_is_free() {
    let mut harness = game_harness();
    let window = Window::start(&mut harness);
    copy_ids(&window, 1);

    // As a native file dialog's modal loop can run the timers inside an action.
    let action = window.app.borrow_mut();
    mock_elapsed_time(TOAST_DURATION);
    drop(action);
    assert_eq!(window.toasts().len(), 1, "waits for the app, not dropped");

    mock_elapsed_time(APP_BUSY_RETRY);
    assert!(window.toasts().is_empty(), "ends once the app is free");
}

#[test]
fn four_quick_toasts_keep_only_the_newest_three() {
    let mut harness = game_harness();
    let window = Window::start(&mut harness);

    window.act(App::copy_database_id);
    copy_ids(&window, MAX_TOASTS);

    assert_eq!(
        window.toast_texts(),
        [
            "Copied TitleID 4D5307E6",
            "Copied DatabaseID 00000012",
            "Copied TitleID 4D5307E6",
        ],
        "the first DatabaseID toast, the oldest, made room"
    );
    assert_eq!(window.cards().len(), MAX_TOASTS);
    mock_elapsed_time(TOAST_DURATION);
    assert!(window.toasts().is_empty(), "each of them ends on time");
}

#[test]
fn a_click_dismisses_a_toast_and_the_window_around_it_stays_usable() {
    let mut harness = game_harness();
    let window = Window::start(&mut harness);
    copy_ids(&window, 2);

    window.cards()[0].mock_single_click(PointerEventButton::Left);

    assert_eq!(window.toast_texts(), ["Copied DatabaseID 00000012"]);
    ElementHandle::find_by_element_id(window.view.window(), "SideBar::console-item")
        .next()
        .expect("the sidebar has a Console row")
        .mock_single_click(PointerEventButton::Left);
    assert_eq!(window.app.borrow().page(), Page::Console);
    assert_eq!(
        window.toasts().len(),
        1,
        "a click elsewhere leaves the toast"
    );
}
