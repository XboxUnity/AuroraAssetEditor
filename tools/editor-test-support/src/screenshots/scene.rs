use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use aurora_asset_editor::app::App;
use aurora_asset_editor::jobs::Job;
use aurora_asset_editor::platform::SystemTheme;
use aurora_asset_editor::settings::ThemeChoice;
use aurora_asset_editor::view::{MainWindowView, SPACE_SETTLE_TIME};
use aurora_ui::MainWindow;
use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use image::RgbaImage;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, SharedString};

use super::screen_text_check::check_screen_text;
use crate::{AppHarness, HeadlessUi, PreviewJobs};

/// How far a capture advances the mock clock: longer than any animation in the UI, the strip's wheel
/// scroll included, and than the wait before pictures are resized to a new space, so every animation has
/// ended, every picture is resized, and the cursor's blink phase is the same on every capture.
pub const SETTLE_TIME: Duration = Duration::from_secs(1);
const CAPTURE_SCALE_FACTOR: f32 = 1.0;
const _: () = assert!(SETTLE_TIME.as_millis() > SPACE_SETTLE_TIME.as_millis());

/// A screen being staged for a screenshot: the harness's app in a connected main window, with everything
/// that could differ between runs pinned (size, theme, scale factor, pointer, mock time).
pub struct Scene {
    ui: HeadlessUi,
    view: MainWindowView,
    previews: PreviewJobs,
    app: Rc<RefCell<App>>,
    harness: AppHarness,
    pointer_kept: bool,
    kept_alive: Vec<Box<dyn Any>>,
}

impl Scene {
    pub fn start(harness: AppHarness, theme: ThemeChoice) -> Scene {
        Scene::start_at_size(harness, theme, HeadlessUi::WINDOW_SIZE)
    }

    pub fn start_at_minimum_size(harness: AppHarness, theme: ThemeChoice) -> Scene {
        Scene::start_at_size(harness, theme, HeadlessUi::MINIMUM_WINDOW_SIZE)
    }

    fn start_at_size(mut harness: AppHarness, theme: ThemeChoice, size: (u32, u32)) -> Scene {
        harness.app().set_theme(theme);
        let ui = HeadlessUi::new();
        let window = MainWindow::new().expect("the main window builds on the testing backend");
        let previews = PreviewJobs::new();
        let view = MainWindowView::new(window, previews.runner());
        ui.show_at_size(view.window().window(), size);
        // The testing backend never reports a dark OS theme; pinned anyway so no screen relies on that.
        view.set_system_theme(SystemTheme { dark: false });
        let app = harness.share_app();
        view.connect(Rc::clone(&app));
        let scene = Scene {
            ui,
            view,
            previews,
            app,
            harness,
            pointer_kept: false,
            kept_alive: Vec::new(),
        };
        scene.render();
        scene
    }

    pub fn act(&self, action: impl FnOnce(&mut App)) {
        action(&mut self.app.borrow_mut());
        self.render();
    }

    pub fn run_jobs(&mut self) {
        self.harness.run_jobs();
        self.render();
    }

    /// The queued job, not run, for a scene that answers it with a synthetic outcome.
    pub fn take_next_job(&mut self) -> Option<Job> {
        self.harness.take_next_job()
    }

    pub fn harness(&mut self) -> &mut AppHarness {
        &mut self.harness
    }

    pub fn window(&self) -> &MainWindow {
        self.view.window()
    }

    pub fn element(&self, id: &str) -> ElementHandle {
        ElementHandle::find_by_element_id(self.window(), id)
            .next()
            .unwrap_or_else(|| panic!("the window has an element {id}"))
    }

    /// Every element with the id, in tree order: repeated elements such as the strip's thumbnails share one.
    pub fn elements(&self, id: &str) -> Vec<ElementHandle> {
        ElementHandle::find_by_element_id(self.window(), id).collect()
    }

    pub fn click(&self, id: &str) {
        self.element(id).mock_single_click(PointerEventButton::Left);
        self.render();
    }

    /// Clicks the element labelled `label` inside the element `container_id`, for rows that have no id.
    pub fn click_label_in(&self, label: &str, container_id: &str) {
        let wanted = label.to_owned();
        self.element(container_id)
            .query_descendants()
            .match_predicate(move |element| {
                element
                    .accessible_label()
                    .is_some_and(|candidate| candidate == wanted.as_str())
            })
            .find_first()
            .unwrap_or_else(|| panic!("{container_id} has an element labelled \"{label}\""))
            .mock_single_click(PointerEventButton::Left);
        self.render();
    }

    pub fn press(&self, key: Key) {
        let text = SharedString::from(key);
        let window = self.window().window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text });
        self.render();
    }

    pub fn type_text(&self, text: &str) {
        self.ui.type_text(self.window().window(), text);
        self.render();
    }

    pub fn dispatch(&self, event: WindowEvent) {
        self.window().window().dispatch_event(event);
        self.render();
    }

    /// Captures with the pointer where the scene left it, for a screen that shows a hover or a drag.
    pub fn keep_pointer_in_place(&mut self) {
        self.pointer_kept = true;
    }

    /// Keeps `value` until the scene ends: a test server or a temp folder the staged state needs.
    pub fn keep_alive(&mut self, value: impl Any) {
        self.kept_alive.push(Box::new(value));
    }

    /// The screen as its screenshot shows it. Panics when the screen shows text that would differ on
    /// another machine, naming the text.
    pub fn capture(&self) -> RgbaImage {
        self.render();
        let window = self.window().window();
        if !self.pointer_kept {
            window.dispatch_event(WindowEvent::PointerExited);
        }
        // Reports the space of every preview area now, so the wait for the spaces to settle ends within
        // SETTLE_TIME and the view asks for the resized pictures.
        mock_elapsed_time(Duration::ZERO);
        mock_elapsed_time(SETTLE_TIME);
        self.previews.run(&self.view);
        self.render();
        assert_eq!(
            self.previews.queued(),
            0,
            "the resized pictures are the ones the screen shows"
        );
        assert_eq!(
            window.scale_factor(),
            CAPTURE_SCALE_FACTOR,
            "screens are captured at scale factor 1"
        );
        if let Err(problem) = check_screen_text(self.window()) {
            panic!("{problem}");
        }
        self.ui.snapshot(window)
    }

    fn render(&self) {
        self.view.render(&self.app.borrow());
    }
}

/// A shown window keeps itself, and with it the view's images, alive until it is hidden; the suite stages
/// every screen on one thread, so each scene hides its window when it ends.
impl Drop for Scene {
    fn drop(&mut self) {
        let Err(error) = self.window().window().hide() else {
            return;
        };
        let problem = format!("the testing backend could not hide a scene's window: {error}");
        // A second panic while a failed staging unwinds would abort the suite and lose its report.
        if std::thread::panicking() {
            eprintln!("{problem}");
        } else {
            panic!("{problem}");
        }
    }
}
