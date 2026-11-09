use std::cell::RefCell;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use slint::PlatformError;

const APP_NAME: &str = "Aurora Asset Editor";
const ABOUT_ID: &str = "about";
const QUIT_ID: &str = "quit";

thread_local! {
    /// What About and Quit run. Menu events are handed to the event loop's thread, where these are set.
    static ACTIONS: RefCell<Option<MenuActions>> = const { RefCell::new(None) };
}

struct MenuActions {
    about: Box<dyn Fn()>,
    quit: Box<dyn Fn()>,
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

/// The macOS app menu: About, Hide, Hide Others, Show All and Quit, running `about` and `quit`. The menu works
/// while this value lives. Quit from the Dock, logout and shutdown still send `terminate:`, which winit handles
/// by ending the app, so they skip the unsaved changes question.
pub struct AppMenu {
    _menu: Menu,
}

impl AppMenu {
    pub fn install(about: impl Fn() + 'static, quit: impl Fn() + 'static) -> muda::Result<AppMenu> {
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
        ACTIONS.set(Some(MenuActions {
            about: Box::new(about),
            quit: Box::new(quit),
        }));
        // This must stay the only menu event handler. muda keeps the first handler set and ignores later ones
        // without an error. Slint sets its own only for a Slint `MenuBar` or a native context menu
        // (`ContextMenuArea`, and the std `LineEdit` and `TextEdit`, which contain one); the UI uses none. Adding
        // one would send its events here, where they are logged and dropped, so it would do nothing on macOS.
        MenuEvent::set_event_handler(Some(hand_over));
        menu.init_for_nsapp();
        Ok(AppMenu { _menu: menu })
    }
}

/// Menu events may arrive on another thread; the item runs in the event loop.
fn hand_over(event: MenuEvent) {
    let id = event.id().0.clone();
    let item = id.clone();
    if let Err(error) = slint::invoke_from_event_loop(move || run(&item)) {
        log::error!("The app menu's {id} item could not reach the event loop: {error}");
    }
}

fn run(id: &str) {
    ACTIONS.with_borrow(|actions| {
        let Some(actions) = actions else {
            log::error!("The app menu's {id} item was chosen before its actions were set");
            return;
        };
        match id {
            ABOUT_ID => (actions.about)(),
            QUIT_ID => (actions.quit)(),
            other => log::error!("The app menu has no item {other}"),
        }
    });
}
