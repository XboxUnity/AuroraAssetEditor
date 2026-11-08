use crate::status::{Severity, StatusLine, ToastId, Toasts};

use super::App;

impl App {
    pub fn toasts(&self) -> &Toasts {
        &self.toasts
    }

    /// The toast's time is up, or it was clicked.
    pub fn dismiss_toast(&mut self, id: ToastId) {
        self.toasts.dismiss(id);
    }

    /// The outcome of a save, an export, a console transfer (a download into the editor included) or a
    /// clipboard copy: in the status line and in a toast.
    pub(super) fn report_done(&mut self, text: String) {
        self.toasts.push(text.clone(), Severity::Info);
        self.status = StatusLine::info(text);
    }

    /// A failure no dialog shows: in the status line and in a toast, both in the error colour.
    pub(super) fn report_failure(&mut self, text: String) {
        self.toasts.push(text.clone(), Severity::Error);
        self.status = StatusLine::error(text);
    }
}
