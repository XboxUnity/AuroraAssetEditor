use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aurora_ui::{ConsoleModel, EditorModel, MainWindow, OnlineModel};
use slint::{ComponentHandle, Timer};

use crate::app::App;
use crate::jobs::{JobRunner, ResizedPreview};
use crate::platform::SystemTheme;

use super::console_view::{connect_console, render_console};
use super::dialog_view::{connect_dialogs, render_dialogs};
use super::editor_view::{EditorPictures, connect_editor, render_editor};
use super::online_view::{OnlinePictures, connect_online, render_online};
use super::preview_view::connect_previews;
use super::shell_view::{Refresh, connect_shell, render_shell};
use super::{GameRows, ImageCache, OnlineImageCache, Previews, ResultTiles, ScreenshotTiles};

/// The window: it shows what `App` holds and turns the window's callbacks into `App` calls.
pub struct MainWindowView {
    window: MainWindow,
    system_theme: Rc<Cell<SystemTheme>>,
    images: Rc<RefCell<ImageCache>>,
    online_images: Rc<RefCell<OnlineImageCache>>,
    previews: Rc<RefCell<Previews>>,
    /// Puts a new space of a preview area to use once it has held still.
    space_settle_timer: Rc<Timer>,
    screenshot_tiles: Rc<RefCell<ScreenshotTiles>>,
    result_tiles: Rc<RefCell<ResultTiles>>,
    game_rows: Rc<RefCell<GameRows>>,
}

impl MainWindowView {
    /// `preview_jobs` runs the jobs that resize the pictures; their outcomes go to `show_resized`.
    pub fn new(window: MainWindow, preview_jobs: Box<dyn JobRunner>) -> MainWindowView {
        let screenshot_tiles = ScreenshotTiles::default();
        window
            .global::<EditorModel>()
            .set_screenshot_tiles(screenshot_tiles.model());
        let result_tiles = ResultTiles::default();
        window
            .global::<OnlineModel>()
            .set_tiles(result_tiles.model());
        let game_rows = GameRows::default();
        window.global::<ConsoleModel>().set_rows(game_rows.model());
        MainWindowView {
            window,
            system_theme: Rc::new(Cell::new(SystemTheme::default())),
            images: Rc::new(RefCell::new(ImageCache::default())),
            online_images: Rc::new(RefCell::new(OnlineImageCache::default())),
            previews: Rc::new(RefCell::new(Previews::new(preview_jobs))),
            space_settle_timer: Rc::new(Timer::default()),
            screenshot_tiles: Rc::new(RefCell::new(screenshot_tiles)),
            result_tiles: Rc::new(RefCell::new(result_tiles)),
            game_rows: Rc::new(RefCell::new(game_rows)),
        }
    }

    pub fn window(&self) -> &MainWindow {
        &self.window
    }

    pub fn system_theme(&self) -> SystemTheme {
        self.system_theme.get()
    }

    /// The OS theme that `ThemeChoice::System` follows; takes effect with the next render.
    pub fn set_system_theme(&self, system_theme: SystemTheme) {
        self.system_theme.set(system_theme);
    }

    /// A picture resized by a preview job; the next render shows it.
    pub fn show_resized(&self, resized: ResizedPreview) {
        self.previews.borrow_mut().store(resized);
    }

    /// Every callback calls its `App` method, then renders.
    pub fn connect(&self, app: Rc<RefCell<App>>) {
        let weak_window = self.window.as_weak();
        let system_theme = Rc::clone(&self.system_theme);
        let images = Rc::clone(&self.images);
        let online_images = Rc::clone(&self.online_images);
        let previews = Rc::clone(&self.previews);
        let screenshot_tiles = Rc::clone(&self.screenshot_tiles);
        let result_tiles = Rc::clone(&self.result_tiles);
        let game_rows = Rc::clone(&self.game_rows);
        let refresh: Refresh = Rc::new(move |app: &App| {
            if let Some(window) = weak_window.upgrade() {
                let caches = Caches {
                    editor: &mut images.borrow_mut(),
                    online: &mut online_images.borrow_mut(),
                    previews: &mut previews.borrow_mut(),
                    screenshot_tiles: &mut screenshot_tiles.borrow_mut(),
                    result_tiles: &mut result_tiles.borrow_mut(),
                    game_rows: &mut game_rows.borrow_mut(),
                };
                render_window(&window, app, system_theme.get(), caches);
            }
        });
        connect_shell(&self.window, &app, &refresh);
        connect_editor(&self.window, &app, &refresh);
        connect_online(&self.window, &app, &refresh);
        connect_console(&self.window, &app, &refresh);
        connect_dialogs(&self.window, &app, &refresh);
        connect_previews(
            &self.window,
            &app,
            &refresh,
            &self.previews,
            &self.space_settle_timer,
        );
    }

    pub fn render(&self, app: &App) {
        let caches = Caches {
            editor: &mut self.images.borrow_mut(),
            online: &mut self.online_images.borrow_mut(),
            previews: &mut self.previews.borrow_mut(),
            screenshot_tiles: &mut self.screenshot_tiles.borrow_mut(),
            result_tiles: &mut self.result_tiles.borrow_mut(),
            game_rows: &mut self.game_rows.borrow_mut(),
        };
        render_window(&self.window, app, self.system_theme.get(), caches);
    }
}

/// What a render reuses from the renders before it: the converted and resized images and the repeated rows.
struct Caches<'a> {
    editor: &'a mut ImageCache,
    online: &'a mut OnlineImageCache,
    previews: &'a mut Previews,
    screenshot_tiles: &'a mut ScreenshotTiles,
    result_tiles: &'a mut ResultTiles,
    game_rows: &'a mut GameRows,
}

fn render_window(window: &MainWindow, app: &App, system_theme: SystemTheme, caches: Caches<'_>) {
    render_shell(window, app, system_theme);
    let mut editor_pictures = EditorPictures {
        originals: caches.editor,
        previews: caches.previews,
    };
    render_editor(window, app, &mut editor_pictures, caches.screenshot_tiles);
    let mut online_pictures = OnlinePictures {
        originals: caches.online,
        previews: caches.previews,
    };
    render_online(window, app, &mut online_pictures, caches.result_tiles);
    caches.previews.forget_unshown();
    render_console(window, app, caches.game_rows);
    render_dialogs(window, app);
}
