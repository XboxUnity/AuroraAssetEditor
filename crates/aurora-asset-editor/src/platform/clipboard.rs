use super::ClipboardError;

pub trait Clipboard {
    fn set_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}
