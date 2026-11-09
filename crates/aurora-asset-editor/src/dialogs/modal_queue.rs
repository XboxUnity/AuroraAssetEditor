use std::collections::VecDeque;

use super::{BulkForm, ConnectionForm, Modal};

/// Modals waiting to be shown. Only the oldest is on screen; the next appears when it closes.
#[derive(Debug, Default)]
pub struct ModalQueue {
    waiting: VecDeque<Modal>,
}

impl ModalQueue {
    pub fn new() -> ModalQueue {
        ModalQueue::default()
    }

    pub fn push(&mut self, modal: Modal) {
        self.waiting.push_back(modal);
    }

    pub fn current(&self) -> Option<&Modal> {
        self.waiting.front()
    }

    pub fn current_mut(&mut self) -> Option<&mut Modal> {
        self.waiting.front_mut()
    }

    /// Whether a modal on screen or waiting matches.
    pub fn any(&self, matches: impl Fn(&Modal) -> bool) -> bool {
        self.waiting.iter().any(matches)
    }

    /// The connection form, wherever it waits in the queue.
    pub fn connection_form_mut(&mut self) -> Option<&mut ConnectionForm> {
        self.waiting.iter_mut().find_map(|modal| match modal {
            Modal::Connection { form } => Some(form),
            Modal::Message { .. }
            | Modal::Choice { .. }
            | Modal::TitleId { .. }
            | Modal::UploadTarget { .. }
            | Modal::Confirm { .. }
            | Modal::Bulk { .. }
            | Modal::About => None,
        })
    }

    /// The bulk form, wherever it waits in the queue.
    pub fn bulk_form_mut(&mut self) -> Option<&mut BulkForm> {
        self.waiting.iter_mut().find_map(|modal| match modal {
            Modal::Bulk { form } => Some(form),
            Modal::Message { .. }
            | Modal::Choice { .. }
            | Modal::TitleId { .. }
            | Modal::UploadTarget { .. }
            | Modal::Confirm { .. }
            | Modal::Connection { .. }
            | Modal::About => None,
        })
    }

    pub fn remove_connection_form(&mut self) {
        self.waiting
            .retain(|modal| !matches!(modal, Modal::Connection { .. }));
    }

    /// Takes the modal on screen away, which shows the next one.
    pub fn close_current(&mut self) -> Option<Modal> {
        self.waiting.pop_front()
    }
}
