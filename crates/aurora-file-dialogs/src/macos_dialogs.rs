use std::io;
use std::path::PathBuf;

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageLevel};

use super::{DialogError, DialogParent, DialogRequest, extension_filters};

const ALL_FILES: &str = "*";

// rfd reports no errors on macOS: a dialog that could not be shown answers like a cancel.

pub fn open_files(request: &DialogRequest) -> Result<Vec<PathBuf>, DialogError> {
    Ok(dialog(request).pick_files().unwrap_or_default())
}

pub fn open_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    Ok(dialog(request).pick_file())
}

pub fn save_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    Ok(dialog(request).save_file())
}

pub fn pick_folder(
    title: &str,
    parent: Option<DialogParent>,
) -> Result<Option<PathBuf>, DialogError> {
    Ok(with_parent(FileDialog::new().set_title(title), parent).pick_folder())
}

pub fn show_startup_error(title: &str, text: &str) -> io::Result<()> {
    // The box has only OK, and rfd reports no failure, so there is nothing to act on.
    let _answer = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(title)
        .set_description(text)
        .set_buttons(MessageButtons::Ok)
        .show();
    Ok(())
}

fn dialog(request: &DialogRequest) -> FileDialog {
    let mut dialog = with_parent(FileDialog::new().set_title(&request.title), request.parent);
    if let Some(name) = &request.default_name {
        dialog = dialog.set_file_name(name);
    }
    let filters = extension_filters(&request.filters, request.default_filter);
    // rfd hands NSOpenPanel one list of allowed types for all filters, so an "All files" entry allows every file.
    let allows_all_files = filters
        .iter()
        .any(|(_, extensions)| extensions.iter().any(|extension| extension == ALL_FILES));
    // Ruled: with "All files" offered, no type list at all, so any file can be picked as on Windows.
    if allows_all_files {
        return dialog;
    }
    for (name, extensions) in filters {
        dialog = dialog.add_filter(name, &extensions);
    }
    dialog
}

fn with_parent(dialog: FileDialog, parent: Option<DialogParent>) -> FileDialog {
    match parent {
        Some(parent) => dialog.set_parent(&parent),
        None => dialog,
    }
}
