use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use aurora_asset_editor::platform::FolderOpener;

/// Records the folders the app asked to show. Like a file manager, it cannot show a folder that does not exist.
pub struct FakeFolderOpener {
    state: Rc<RefCell<FolderOpenerState>>,
}

#[derive(Default)]
struct FolderOpenerState {
    opened: Vec<PathBuf>,
    failure: Option<String>,
}

#[derive(Clone)]
pub struct FakeFolderOpenerHandle {
    state: Rc<RefCell<FolderOpenerState>>,
}

impl FakeFolderOpener {
    pub fn new() -> (FakeFolderOpener, FakeFolderOpenerHandle) {
        let state = Rc::new(RefCell::new(FolderOpenerState::default()));
        (
            FakeFolderOpener {
                state: Rc::clone(&state),
            },
            FakeFolderOpenerHandle { state },
        )
    }
}

impl FakeFolderOpenerHandle {
    pub fn opened(&self) -> Vec<PathBuf> {
        self.state.borrow().opened.clone()
    }

    pub fn fail_with(&self, message: &str) {
        self.state.borrow_mut().failure = Some(message.to_owned());
    }
}

impl FolderOpener for FakeFolderOpener {
    fn open_folder(&mut self, path: &Path) -> io::Result<()> {
        let mut state = self.state.borrow_mut();
        if let Some(message) = &state.failure {
            return Err(io::Error::other(message.clone()));
        }
        if !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            ));
        }
        state.opened.push(path.to_path_buf());
        Ok(())
    }
}
