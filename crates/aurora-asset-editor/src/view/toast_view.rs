use std::cell::RefCell;
use std::rc::Rc;

use aurora_ui::{MainWindow, ShellModel, ToastView};
use slint::ComponentHandle;

use crate::app::App;
use crate::status::Severity;

use super::shell_view::{Refresh, action_runner};
use super::{ToastRows, ToastTimers};

/// Connects a click on a toast and the end of a toast's time to `app`.
pub(super) fn connect_toasts(
    window: &MainWindow,
    app: &Rc<RefCell<App>>,
    refresh: &Refresh,
    timers: &RefCell<ToastTimers>,
) {
    let act = action_runner(app, refresh);
    let shell = window.global::<ShellModel>();
    shell.on_dismiss_toast(move |position| act(&mut |app| dismiss_toast_at(app, position)));

    // Weak, since the timers belong to the window that `refresh` renders.
    let app = Rc::downgrade(app);
    let refresh = Rc::downgrade(refresh);
    timers.borrow_mut().connect(Rc::new(move |id| {
        // Gone only while the window closes, and its toasts with it.
        let (Some(app), Some(refresh)) = (app.upgrade(), refresh.upgrade()) else {
            return;
        };
        let mut app = app.borrow_mut();
        app.dismiss_toast(id);
        refresh(&app);
    }));
}

pub(super) fn render_toasts(app: &App, rows: &mut ToastRows, timers: &mut ToastTimers) {
    let toasts: Vec<ToastView> = app
        .toasts()
        .iter()
        .map(|toast| ToastView {
            text: toast.text.as_str().into(),
            error: toast.severity == Severity::Error,
        })
        .collect();
    rows.show(toasts, Clone::clone);
    timers.track(app.toasts());
}

/// A click on the toast at `position`, counted from the oldest.
fn dismiss_toast_at(app: &mut App, position: i32) {
    let toast = usize::try_from(position)
        .ok()
        .and_then(|position| app.toasts().get(position))
        .map(|toast| toast.id);
    match toast {
        Some(id) => app.dismiss_toast(id),
        None => log::error!("Toast {position} was clicked, which the window does not show"),
    }
}
