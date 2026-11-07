use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use aurora_asset_editor::platform::FolderOpener;

/// Records the folders the app asked to show.
pub struct FakeFolderOpener {
    opened: Rc<RefCell<Vec<PathBuf>>>,
}

#[derive(Clone)]
pub struct FakeFolderOpenerHandle {
    opened: Rc<RefCell<Vec<PathBuf>>>,
}

impl FakeFolderOpener {
    pub fn new() -> (FakeFolderOpener, FakeFolderOpenerHandle) {
        let opened = Rc::new(RefCell::new(Vec::new()));
        (
            FakeFolderOpener {
                opened: Rc::clone(&opened),
            },
            FakeFolderOpenerHandle { opened },
        )
    }
}

impl FakeFolderOpenerHandle {
    pub fn opened(&self) -> Vec<PathBuf> {
        self.opened.borrow().clone()
    }
}

impl FolderOpener for FakeFolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()> {
        self.opened.borrow_mut().push(path.to_path_buf());
        Ok(())
    }
}
