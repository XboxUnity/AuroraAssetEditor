use std::collections::VecDeque;
use std::time::Duration;

use super::Severity;

/// How long a toast shows before it goes by itself.
pub const TOAST_DURATION: Duration = Duration::from_secs(4);
/// The most toasts on screen at once; a new one beyond it pushes out the oldest.
pub const MAX_TOASTS: usize = 3;

/// Names one toast for as long as the app runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

/// The outcome of an action, shown for a moment over the bottom right of the window as well as in the status
/// line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: ToastId,
    pub text: String,
    pub severity: Severity,
}

/// The toasts on screen, oldest first.
#[derive(Debug, Default)]
pub struct Toasts {
    shown: VecDeque<Toast>,
    next_id: u64,
}

impl Toasts {
    pub fn push(&mut self, text: String, severity: Severity) {
        let id = ToastId(self.next_id);
        self.next_id += 1;
        self.shown.push_back(Toast { id, text, severity });
        while self.shown.len() > MAX_TOASTS {
            self.shown.pop_front();
        }
    }

    pub fn dismiss(&mut self, id: ToastId) {
        self.shown.retain(|toast| toast.id != id);
    }

    /// The toast at `position`, counted from the oldest.
    pub fn get(&self, position: usize) -> Option<&Toast> {
        self.shown.get(position)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.shown.iter()
    }

    pub fn contains(&self, id: ToastId) -> bool {
        self.shown.iter().any(|toast| toast.id == id)
    }
}
