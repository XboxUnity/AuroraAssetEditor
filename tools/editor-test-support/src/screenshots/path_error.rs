use std::io;
use std::path::Path;

/// `error` with `path` in front of its message, keeping its kind.
pub(super) fn with_path(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}
