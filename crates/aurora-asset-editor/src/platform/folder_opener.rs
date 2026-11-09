use std::io;
use std::path::Path;

pub trait FolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()>;
}
