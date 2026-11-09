use std::io;
use std::path::Path;

use super::FolderOpener;

#[derive(Default)]
pub struct SystemFolderOpener;

impl FolderOpener for SystemFolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()> {
        open::that(path)
    }
}
