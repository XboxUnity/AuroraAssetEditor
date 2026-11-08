use std::cell::RefCell;
use std::rc::Weak;
use std::time::Duration;

use slint::Timer;

use crate::app::App;

/// How long a timer's action waits before it tries again while a window action still holds the app.
pub const APP_BUSY_RETRY: Duration = Duration::from_millis(50);

/// Runs `action` on the app for a timer. A timer can fire while a window action still holds the app: on macOS
/// a native file dialog runs a modal loop inside the action, and Slint's display link, which ticks in every
/// run loop mode, runs the timers in it. The action then tries again `APP_BUSY_RETRY` later, until the app
/// is free; it is never dropped. Once the app is gone with its window, nothing runs.
pub fn run_when_app_free(app: Weak<RefCell<App>>, action: impl FnOnce(&mut App) + 'static) {
    let Some(shared) = app.upgrade() else {
        return;
    };
    let Ok(mut free) = shared.try_borrow_mut() else {
        log::debug!("A timer found the app busy and tries again");
        Timer::single_shot(APP_BUSY_RETRY, move || run_when_app_free(app, action));
        return;
    };
    action(&mut free);
}
