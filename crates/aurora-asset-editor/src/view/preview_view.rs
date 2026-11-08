use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use aurora_ui::{MainWindow, PreviewModel};
use slint::{ComponentHandle, Timer, TimerMode};

use crate::app::App;

use super::Previews;
use super::app_when_free::run_when_app_free;
use super::shell_view::Refresh;

/// How long an area's space must hold still before its pictures are resized to it, so dragging the window's
/// edge resizes each picture once the drag pauses, not at every step.
pub const SPACE_SETTLE_TIME: Duration = Duration::from_millis(100);

/// Connects the areas' space reports to `previews`: a new space is put to use, and the window rendered, once it
/// has held still for `SPACE_SETTLE_TIME`.
pub(super) fn connect_previews(
    window: &MainWindow,
    app: &Rc<RefCell<App>>,
    refresh: &Refresh,
    previews: &Rc<RefCell<Previews>>,
    settle_timer: &Rc<Timer>,
) {
    let app = Rc::clone(app);
    let refresh = Rc::clone(refresh);
    let previews = Rc::clone(previews);
    let settle_timer = Rc::clone(settle_timer);
    let preview_model = window.global::<PreviewModel>();
    preview_model.on_area_changed(move |area, width, height| {
        // An area that is not laid out yet has no space to fit.
        let (Ok(width @ 1..), Ok(height @ 1..)) = (u32::try_from(width), u32::try_from(height))
        else {
            return;
        };
        if !previews.borrow_mut().report_space(area, (width, height)) {
            return;
        }
        let app = Rc::downgrade(&app);
        let refresh = Rc::clone(&refresh);
        let previews = Rc::clone(&previews);
        settle_timer.start(TimerMode::SingleShot, SPACE_SETTLE_TIME, move || {
            let refresh = Rc::clone(&refresh);
            let previews = Rc::clone(&previews);
            run_when_app_free(Weak::clone(&app), move |app| {
                previews.borrow_mut().settle_spaces();
                refresh(app);
            });
        });
    });
    preview_model.set_view_listening(true);
}
