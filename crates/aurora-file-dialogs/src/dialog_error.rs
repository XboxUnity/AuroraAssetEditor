use std::error::Error;
use std::fmt;

/// The system could not show a file or folder dialog, or could not say what the user picked.
#[derive(Debug)]
pub struct DialogError {
    source: Box<dyn Error + Send + Sync>,
}

impl DialogError {
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> DialogError {
        DialogError {
            source: source.into(),
        }
    }
}

impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the system file dialog failed")
    }
}

impl Error for DialogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
