use std::path::PathBuf;

use aurora_file_dialogs::{DialogFilter, DialogParent, DialogRequest};
use aurora_ui::MainWindow;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use slint::ComponentHandle;
use slint::Weak;

use super::{DialogError, FileDialogs, FileFilter, OpenRequest, SaveRequest};

/// The system's file dialogs, each modal to the main window.
pub struct NativeFileDialogs {
    window: Weak<MainWindow>,
}

impl NativeFileDialogs {
    pub fn new(window: Weak<MainWindow>) -> NativeFileDialogs {
        NativeFileDialogs { window }
    }

    /// Runs `show` with the main window as the dialog's parent.
    fn with_parent<T>(
        &self,
        show: impl FnOnce(DialogParent) -> Result<T, aurora_file_dialogs::DialogError>,
    ) -> Result<T, DialogError> {
        let window = self
            .window
            .upgrade()
            .ok_or_else(|| dialog_error("the main window is closed"))?;
        let handle = window.window().window_handle();
        let parent = DialogParent {
            window: handle.window_handle().map_err(dialog_error)?,
            display: handle.display_handle().map_err(dialog_error)?,
        };
        show(parent).map_err(dialog_error)
    }
}

impl FileDialogs for NativeFileDialogs {
    fn open_files(&mut self, request: &OpenRequest) -> Result<Vec<PathBuf>, DialogError> {
        self.with_parent(|parent| {
            aurora_file_dialogs::open_files(&dialog_request(
                &request.title,
                &request.filters,
                request.default_filter,
                &request.default_name,
                parent,
            ))
        })
    }

    fn open_file(&mut self, request: &OpenRequest) -> Result<Option<PathBuf>, DialogError> {
        self.with_parent(|parent| {
            aurora_file_dialogs::open_file(&dialog_request(
                &request.title,
                &request.filters,
                request.default_filter,
                &request.default_name,
                parent,
            ))
        })
    }

    fn save_file(&mut self, request: &SaveRequest) -> Result<Option<PathBuf>, DialogError> {
        self.with_parent(|parent| {
            aurora_file_dialogs::save_file(&dialog_request(
                &request.title,
                &request.filters,
                request.default_filter,
                &request.default_name,
                parent,
            ))
        })
    }

    fn pick_folder(&mut self, title: &str) -> Result<Option<PathBuf>, DialogError> {
        self.with_parent(|parent| aurora_file_dialogs::pick_folder(title, Some(parent)))
    }
}

fn dialog_request<'a>(
    title: &str,
    filters: &[FileFilter],
    default_filter: usize,
    default_name: &Option<String>,
    parent: DialogParent<'a>,
) -> DialogRequest<'a> {
    DialogRequest {
        title: title.to_owned(),
        filters: filters
            .iter()
            .map(|filter| DialogFilter {
                name: filter.name.clone(),
                patterns: filter.patterns.clone(),
            })
            .collect(),
        default_filter,
        default_name: default_name.clone(),
        parent: Some(parent),
    }
}

fn dialog_error(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> DialogError {
    DialogError {
        source: source.into(),
    }
}
