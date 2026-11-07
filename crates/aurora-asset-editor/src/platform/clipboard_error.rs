use std::error::Error;
use std::fmt;

/// The system clipboard refused the text. `source` holds the platform's own error.
#[derive(Debug)]
pub struct ClipboardError {
    pub source: Box<dyn Error + Send + Sync>,
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("could not use the clipboard")
    }
}

impl Error for ClipboardError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
