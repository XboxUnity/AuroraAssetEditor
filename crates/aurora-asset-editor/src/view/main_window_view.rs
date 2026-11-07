use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aurora_ui::MainWindow;
use slint::ComponentHandle;

use crate::app::App;
use crate::platform::SystemTheme;

use super::ImageCache;
use super::editor_view::{connect_editor, render_editor};
use super::shell_view::{Refresh, connect_shell, render_shell};

/// The window: it shows what `App` holds and turns the window's callbacks into `App` calls.
pub struct MainWindowView {
    window: MainWindow,
    system_theme: Rc<Cell<SystemTheme>>,
    images: Rc<RefCell<ImageCache>>,
}

impl MainWindowView {
    pub fn new(window: MainWindow) -> MainWindowView {
        MainWindowView {
            window,
            system_theme: Rc::new(Cell::new(SystemTheme::default())),
            images: Rc::new(RefCell::new(ImageCache::default())),
        }
    }

    pub fn window(&self) -> &MainWindow {
        &self.window
    }

    /// The OS theme that `ThemeChoice::System` follows; takes effect with the next render.
    pub fn set_system_theme(&self, system_theme: SystemTheme) {
        self.system_theme.set(system_theme);
    }

    /// Every callback calls its `App` method, then renders.
    pub fn connect(&self, app: Rc<RefCell<App>>) {
        let weak_window = self.window.as_weak();
        let system_theme = Rc::clone(&self.system_theme);
        let images = Rc::clone(&self.images);
        let refresh: Refresh = Rc::new(move |app: &App| {
            if let Some(window) = weak_window.upgrade() {
                render_window(&window, app, system_theme.get(), &mut images.borrow_mut());
            }
        });
        connect_shell(&self.window, &app, &refresh);
        connect_editor(&self.window, &app, &refresh);
    }

    pub fn render(&self, app: &App) {
        render_window(
            &self.window,
            app,
            self.system_theme.get(),
            &mut self.images.borrow_mut(),
        );
    }
}

fn render_window(
    window: &MainWindow,
    app: &App,
    system_theme: SystemTheme,
    images: &mut ImageCache,
) {
    render_shell(window, app, system_theme);
    render_editor(window, app, images);
}
