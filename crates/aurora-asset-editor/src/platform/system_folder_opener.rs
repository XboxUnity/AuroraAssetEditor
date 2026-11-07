use std::io;
use std::path::Path;

use super::FolderOpener;

/// Opens folders in the system's file manager.
#[derive(Default)]
pub struct SystemFolderOpener;

impl FolderOpener for SystemFolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()> {
        open::that(path)
    }
}
