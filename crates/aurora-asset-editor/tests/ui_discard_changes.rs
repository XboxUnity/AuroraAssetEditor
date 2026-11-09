use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aurora_asset::{AssetKind, AssetSlot, Compression, RgbaImage};
use aurora_asset_editor::app::App;
use aurora_asset_editor::view::MainWindowView;
use aurora_ui::{DialogKind, DialogModel, MainWindow};
use editor_test_support::{AppHarness, HeadlessUi, PreviewJobs, asset_file_with};
use i_slint_backend_testing::ElementHandle;
use image::Rgba;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, SharedString};

const CONFIRM_BUTTON: &str = "ConfirmDialog::confirm-button";
const CANCEL_BUTTON: &str = "ConfirmDialog::cancel-button";
const DANGER: [u8; 3] = [0xdc, 0x26, 0x26];
const FOCUS_OUTLINE: [u8; 3] = [0x3b, 0x82, 0xf6];
const COLOUR_TOLERANCE: i16 = 3;
/// Inside a button's left padding, clear of its rounded corners and its focus outline.
const BUTTON_PROBE_INSET: f32 = 3.0;

/// A window whose close requests go through the app, as `main` connects them.
struct Ui {
    ui: HeadlessUi,
    view: Rc<MainWindowView>,
    app: Rc<RefCell<App>>,
    /// How often the window ran `before_close`.
    closed: Rc<Cell<usize>>,
}

impl Ui {
    fn start(harness: &mut AppHarness) -> Ui {
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let view = Rc::new(MainWindowView::new(window, PreviewJobs::new().runner()));
        ui.show(view.window().window());
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let closed = Rc::new(Cell::new(0));
        let counted = Rc::clone(&closed);
        view.connect_close(&app, move |_, _| counted.set(counted.get() + 1));
        view.render(&app.borrow());
        Ui {
            ui,
            view,
            app,
            closed,
        }
    }

    fn act(&self, action: impl FnOnce(&mut App)) {
        action(&mut self.app.borrow_mut());
        self.view.render(&self.app.borrow());
    }

    fn window(&self) -> &slint::Window {
        self.view.window().window()
    }

    fn dialog(&self) -> DialogModel<'_> {
        self.view.window().global::<DialogModel>()
    }

    fn press(&self, key: Key) {
        let text = SharedString::from(key);
        self.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        self.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }

    fn click(&self, id: &str) {
        self.element(id).mock_single_click(PointerEventButton::Left);
    }

    fn element(&self, id: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(self.view.window(), id)
            .next()
            .unwrap_or_else(|| panic!("the window shows {id}"))
    }

    /// The colour at the left edge of the button `id` (`inset` 0, where a focus outline is drawn) or inside it.
    fn button_colour(&self, id: &str, inset: f32) -> [u8; 3] {
        let button = self.element(id);
        let (position, size) = (button.absolute_position(), button.size());
        let snapshot = self.ui.snapshot(self.window());
        let pixel = snapshot.get_pixel(
            (position.x + inset) as u32,
            (position.y + size.height / 2.0) as u32,
        );
        [pixel[0], pixel[1], pixel[2]]
    }

    fn unsaved(&self) -> Vec<AssetKind> {
        self.app.borrow().pages().unsaved_among(&AssetKind::ALL)
    }
}

fn assert_close(actual: [u8; 3], expected: [u8; 3], what: &str) {
    let close = actual.iter().zip(expected).all(|(actual, expected)| {
        (i16::from(*actual) - i16::from(expected)).abs() <= COLOUR_TOLERANCE
    });
    assert!(close, "{what}: {actual:?} is not close to {expected:?}");
}

/// The Cover page opened from a file, then its cover removed.
fn edited_harness() -> AppHarness {
    let mut harness = AppHarness::new();
    let mut cover = asset_file_with(&[(
        AssetSlot::Cover,
        RgbaImage::from_pixel(8, 8, Rgba([200, 30, 30, 255])),
        Compression::None,
    )]);
    let path = harness.config_dir().join("GC4D5307E6.asset");
    std::fs::write(&path, cover.to_bytes()).expect("the temp folder is writable");
    harness.app().open_paths(vec![path]);
    harness.run_jobs();
    harness.app().remove_image(AssetSlot::Cover);
    harness
}

#[test]
fn the_discard_question_is_a_confirmation_whose_default_is_cancel() {
    let mut harness = edited_harness();
    let ui = Ui::start(&mut harness);

    ui.act(|app| app.new_asset_set());
    assert_eq!(ui.dialog().get_kind(), DialogKind::Confirm);
    assert_eq!(ui.dialog().get_title(), "Unsaved changes");
    assert_eq!(ui.dialog().get_text(), "Discard unsaved changes to Cover?");
    assert_eq!(ui.dialog().get_confirm_label(), "Discard");
    assert_close(
        ui.button_colour(CONFIRM_BUTTON, BUTTON_PROBE_INSET),
        DANGER,
        "Discard",
    );
    assert_close(
        ui.button_colour(CANCEL_BUTTON, 0.0),
        FOCUS_OUTLINE,
        "Cancel has the focus",
    );

    ui.press(Key::Return);
    assert_eq!(ui.app.borrow().modal(), None, "Return answers Cancel");
    assert_eq!(ui.unsaved(), vec![AssetKind::Cover]);

    ui.act(|app| app.new_asset_set());
    ui.press(Key::Escape);
    assert_eq!(ui.app.borrow().modal(), None, "Escape answers Cancel");
    assert_eq!(ui.unsaved(), vec![AssetKind::Cover]);

    ui.act(|app| app.new_asset_set());
    ui.click(CONFIRM_BUTTON);
    assert_eq!(ui.app.borrow().modal(), None);
    assert_eq!(ui.unsaved(), Vec::new(), "Discard ran New asset set");
}

#[test]
fn closing_the_window_asks_and_only_discard_closes_it() {
    let mut harness = edited_harness();
    let ui = Ui::start(&mut harness);

    ui.window().dispatch_event(WindowEvent::CloseRequested);
    assert!(ui.window().is_visible(), "the window waits for the answer");
    assert_eq!(ui.dialog().get_kind(), DialogKind::Confirm);
    assert_eq!(ui.dialog().get_text(), "Discard unsaved changes to Cover?");

    ui.press(Key::Escape);
    assert!(ui.window().is_visible(), "Cancel keeps the window open");
    assert_eq!(ui.dialog().get_kind(), DialogKind::None);
    assert_eq!(ui.closed.get(), 0);

    ui.window().dispatch_event(WindowEvent::CloseRequested);
    ui.click(CONFIRM_BUTTON);
    assert!(!ui.window().is_visible(), "Discard closes the window");
    assert_eq!(
        ui.closed.get(),
        1,
        "the window closed once, as main closes it"
    );
}

#[test]
fn closing_the_window_without_unsaved_changes_does_not_ask() {
    let mut harness = AppHarness::new();
    let ui = Ui::start(&mut harness);

    ui.window().dispatch_event(WindowEvent::CloseRequested);

    assert!(!ui.window().is_visible());
    assert_eq!(ui.dialog().get_kind(), DialogKind::None);
    assert_eq!(ui.closed.get(), 1);
}
