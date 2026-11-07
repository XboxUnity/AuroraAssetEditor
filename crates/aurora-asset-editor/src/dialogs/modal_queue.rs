use std::collections::VecDeque;

use super::Modal;

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

    /// Takes the modal on screen away, which shows the next one.
    pub fn close_current(&mut self) -> Option<Modal> {
        self.waiting.pop_front()
    }
}
