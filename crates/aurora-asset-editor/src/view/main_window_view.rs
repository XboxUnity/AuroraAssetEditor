use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aurora_ui::MainWindow;
use slint::ComponentHandle;

use crate::app::App;
use crate::platform::SystemTheme;

use super::console_view::{connect_console, render_console};
use super::editor_view::{connect_editor, render_editor};
use super::online_view::{connect_online, render_online};
use super::shell_view::{Refresh, connect_shell, render_shell};
use super::{ImageCache, OnlineImageCache};

/// The window: it shows what `App` holds and turns the window's callbacks into `App` calls.
pub struct MainWindowView {
    window: MainWindow,
    system_theme: Rc<Cell<SystemTheme>>,
    images: Rc<RefCell<ImageCache>>,
    online_images: Rc<RefCell<OnlineImageCache>>,
}

impl MainWindowView {
    pub fn new(window: MainWindow) -> MainWindowView {
        MainWindowView {
            window,
            system_theme: Rc::new(Cell::new(SystemTheme::default())),
            images: Rc::new(RefCell::new(ImageCache::default())),
            online_images: Rc::new(RefCell::new(OnlineImageCache::default())),
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
        let online_images = Rc::clone(&self.online_images);
        let refresh: Refresh = Rc::new(move |app: &App| {
            if let Some(window) = weak_window.upgrade() {
                let caches = Caches {
                    editor: &mut images.borrow_mut(),
                    online: &mut online_images.borrow_mut(),
                };
                render_window(&window, app, system_theme.get(), caches);
            }
        });
        connect_shell(&self.window, &app, &refresh);
        connect_editor(&self.window, &app, &refresh);
        connect_online(&self.window, &app, &refresh);
        connect_console(&self.window, &app, &refresh);
    }

    pub fn render(&self, app: &App) {
        let caches = Caches {
            editor: &mut self.images.borrow_mut(),
            online: &mut self.online_images.borrow_mut(),
        };
        render_window(&self.window, app, self.system_theme.get(), caches);
    }
}

/// The converted images a render reuses.
struct Caches<'a> {
    editor: &'a mut ImageCache,
    online: &'a mut OnlineImageCache,
}

fn render_window(window: &MainWindow, app: &App, system_theme: SystemTheme, caches: Caches<'_>) {
    render_shell(window, app, system_theme);
    render_editor(window, app, caches.editor);
    render_online(window, app, caches.online);
    render_console(window, app);
}
