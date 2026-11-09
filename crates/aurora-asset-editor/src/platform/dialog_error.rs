use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct DialogError {
    pub source: Box<dyn Error + Send + Sync>,
}

impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("could not open the file dialog")
    }
}

impl Error for DialogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}
