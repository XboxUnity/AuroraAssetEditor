use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use super::run_manifest::{MANIFEST_FILE, MANIFEST_FORMAT};

#[derive(Debug)]
pub enum ManifestError {
    Missing(PathBuf),
    Unreadable { path: PathBuf, error: io::Error },
    UnknownFormat(String),
    BadLine { line: usize, text: String },
    MissingField(&'static str),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => {
                write!(formatter, "{MANIFEST_FILE} is missing: {}", path.display())
            }
            Self::Unreadable { path, error } => write!(
                formatter,
                "{MANIFEST_FILE} could not be read ({}): {error}",
                path.display()
            ),
            Self::UnknownFormat(found) => write!(
                formatter,
                "{MANIFEST_FILE} has format {found}, this tool reads format {MANIFEST_FORMAT}"
            ),
            Self::BadLine { line, text } => write!(
                formatter,
                "{MANIFEST_FILE} line {line} is not a known \"key: value\" line: {text}"
            ),
            Self::MissingField(field) => {
                write!(formatter, "{MANIFEST_FILE} has no \"{field}\" line")
            }
        }
    }
}

impl Error for ManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { error, .. } => Some(error),
            _ => None,
        }
    }
}
