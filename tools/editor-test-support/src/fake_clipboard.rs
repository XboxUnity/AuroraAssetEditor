use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use aurora_asset_editor::platform::{Clipboard, ClipboardError};

#[derive(Default)]
struct ClipboardState {
    text: Option<String>,
    failure: Option<String>,
}

/// Keeps the copied text for the test, or fails every copy once `fail_with` was called.
pub struct FakeClipboard {
    state: Rc<RefCell<ClipboardState>>,
}

#[derive(Clone)]
pub struct FakeClipboardHandle {
    state: Rc<RefCell<ClipboardState>>,
}

impl FakeClipboard {
    pub fn new() -> (FakeClipboard, FakeClipboardHandle) {
        let state = Rc::new(RefCell::new(ClipboardState::default()));
        (
            FakeClipboard {
                state: Rc::clone(&state),
            },
            FakeClipboardHandle { state },
        )
    }
}

impl FakeClipboardHandle {
    pub fn text(&self) -> Option<String> {
        self.state.borrow().text.clone()
    }

    /// Every later copy fails with an error whose source carries `message`.
    pub fn fail_with(&self, message: &str) {
        self.state.borrow_mut().failure = Some(message.to_owned());
    }
}

impl Clipboard for FakeClipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let mut state = self.state.borrow_mut();
        if let Some(message) = &state.failure {
            return Err(ClipboardError {
                source: Box::new(io::Error::other(message.clone())),
            });
        }
        state.text = Some(text.to_owned());
        Ok(())
    }
}
