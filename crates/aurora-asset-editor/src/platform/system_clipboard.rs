use super::{Clipboard, ClipboardError};

/// The system clipboard. The handle is made on first use and kept, because on X11 the text stays available
/// only while the program that copied it keeps serving it.
#[derive(Default)]
pub struct SystemClipboard {
    clipboard: Option<arboard::Clipboard>,
}

impl Clipboard for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let clipboard = match self.clipboard.take() {
            Some(clipboard) => clipboard,
            None => arboard::Clipboard::new().map_err(clipboard_error)?,
        };
        self.clipboard
            .insert(clipboard)
            .set_text(text)
            .map_err(clipboard_error)
    }
}

fn clipboard_error(error: arboard::Error) -> ClipboardError {
    ClipboardError {
        source: Box::new(error),
    }
}
