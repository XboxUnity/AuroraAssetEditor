use std::io;
use std::path::Path;

/// Shows a folder in the system's file manager.
pub trait FolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()>;
}
