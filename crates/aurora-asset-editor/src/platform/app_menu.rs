use std::cell::RefCell;

use aurora_ui::MainWindow;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PlatformError, Weak};

const APP_NAME: &str = "Aurora Asset Editor";
const ABOUT_ID: &str = "about";
const QUIT_ID: &str = "quit";

thread_local! {
    /// What About runs. Menu events are handed to the event loop's thread, where this is set.
    static ABOUT_ACTION: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// The winit backend without Slint's default macOS menu bar. That menu's Quit sends `terminate:`, which ends the
/// app without a close request, so the question about unsaved changes and the window geometry save would be
/// skipped. Must run before the first window is created.
pub fn use_backend_without_default_menu() -> Result<(), PlatformError> {
    let backend = i_slint_backend_winit::Backend::builder()
        .with_default_menu_bar(false)
        .build()?;
    slint::platform::set_platform(Box::new(backend)).map_err(PlatformError::SetPlatformError)
}

/// The macOS app menu: About, Hide, Hide Others, Show All and Quit. Quit asks the window to close the way its
/// close button does, About runs `about`. The menu works while this value lives.
pub struct AppMenu {
    _menu: Menu,
}

impl AppMenu {
    /// Slint installs its own menu event handler only for a Slint `MenuBar` or a native context menu, which the
    /// app does not use, so this handler receives every menu event.
    pub fn install(window: Weak<MainWindow>, about: impl Fn() + 'static) -> muda::Result<AppMenu> {
        let about_item = MenuItem::with_id(ABOUT_ID, format!("About {APP_NAME}"), true, None);
        let hide_text = format!("Hide {APP_NAME}");
        let quit_item = MenuItem::with_id(
            QUIT_ID,
            format!("Quit {APP_NAME}"),
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyQ)),
        );
        let app_menu = Submenu::with_items(
            APP_NAME,
            true,
            &[
                &about_item,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(Some(&hide_text)),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &quit_item,
            ],
        )?;
        let menu = Menu::with_items(&[&app_menu])?;
        ABOUT_ACTION.set(Some(Box::new(about)));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| hand_over(&window, event)));
        menu.init_for_nsapp();
        Ok(AppMenu { _menu: menu })
    }
}

/// Menu events may arrive on another thread; the item runs in the event loop.
fn hand_over(window: &Weak<MainWindow>, event: MenuEvent) {
    let id = event.id().0.clone();
    let window = window.clone();
    let item = id.clone();
    if let Err(error) = slint::invoke_from_event_loop(move || run(&window, &item)) {
        log::error!("The app menu's {id} item could not reach the event loop: {error}");
    }
}

fn run(window: &Weak<MainWindow>, id: &str) {
    match id {
        QUIT_ID => match window.upgrade() {
            Some(window) => window.window().dispatch_event(WindowEvent::CloseRequested),
            None => log::error!("Quit was chosen after the window was gone"),
        },
        ABOUT_ID => ABOUT_ACTION.with_borrow(|about| match about {
            Some(about) => about(),
            None => log::error!("About was chosen before the app menu was installed"),
        }),
        other => log::error!("The app menu has no item {other}"),
    }
}
