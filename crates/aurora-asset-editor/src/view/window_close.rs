use std::cell::RefCell;

use slint::platform::WindowEvent;

use crate::app::App;

pub const CLOSE_FAILED: &str = "Could not close the window";

/// Asks `window` to close the way the system does: through its close-request handler, which asks the app
/// first. When the window cannot be hidden, the failure is logged, and shown in the status line and a red
/// toast while the window is still up.
pub fn request_window_close(window: &slint::Window, app: &RefCell<App>) {
    let Err(error) = window.dispatch_event_with_result(WindowEvent::CloseRequested) else {
        return;
    };
    if !window.is_visible() {
        log::error!("{CLOSE_FAILED}: {error}");
        return;
    }
    match app.try_borrow_mut() {
        Ok(mut app) => app.report_view_failure(CLOSE_FAILED, &error),
        Err(_held_by_an_action) => {
            log::error!("{CLOSE_FAILED} while an action held the app: {error}");
        }
    }
}
