use std::collections::HashMap;
use std::rc::Rc;

use slint::{Timer, TimerMode};

use crate::status::{TOAST_DURATION, ToastId, Toasts};

/// Ends each toast `TOAST_DURATION` after it first shows. Slint timers run on the window's clock, which tests
/// advance with mock time.
#[derive(Default)]
pub struct ToastTimers {
    running: HashMap<ToastId, Timer>,
    expire: Option<Rc<dyn Fn(ToastId)>>,
}

impl ToastTimers {
    /// `expire` ends a toast whose time is up. Toasts shown before it is set start their time at the next
    /// `track`.
    pub fn connect(&mut self, expire: Rc<dyn Fn(ToastId)>) {
        self.expire = Some(expire);
    }

    /// Starts the time of each toast shown for the first time; a toast that is gone takes its timer with it.
    pub fn track(&mut self, toasts: &Toasts) {
        self.running.retain(|id, _| toasts.contains(*id));
        let Some(expire) = &self.expire else {
            return;
        };
        for toast in toasts.iter() {
            self.running.entry(toast.id).or_insert_with(|| {
                let expire = Rc::clone(expire);
                let id = toast.id;
                let timer = Timer::default();
                timer.start(TimerMode::SingleShot, TOAST_DURATION, move || expire(id));
                timer
            });
        }
    }
}
