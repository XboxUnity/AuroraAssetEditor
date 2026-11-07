// The Windows file dialogs and message box are Win32 APIs, which the `windows` crate only
// offers as `unsafe` calls.
#![deny(unsafe_code)]

mod dialog_error;
mod dialog_filter;
mod dialog_request;
mod extension_filters;
// Only the Linux portal answers with file URIs; the module is pure so its tests run everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod file_uri;
#[cfg(target_os = "macos")]
mod macos_dialogs;
#[cfg(target_os = "linux")]
mod portal_dialogs;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_dialogs;

use std::io;
use std::path::PathBuf;

pub use dialog_error::DialogError;
pub use dialog_filter::DialogFilter;
pub use dialog_request::{DialogParent, DialogRequest};
pub use extension_filters::extension_filters;

#[cfg(target_os = "macos")]
use macos_dialogs as platform;
#[cfg(target_os = "linux")]
use portal_dialogs as platform;
#[cfg(windows)]
use windows_dialogs as platform;

/// The files the user picked; an empty list is the user's cancel.
pub fn open_files(request: &DialogRequest) -> Result<Vec<PathBuf>, DialogError> {
    platform::open_files(request)
}

/// The file the user picked; `None` is the user's cancel.
pub fn open_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    platform::open_file(request)
}

/// Where the user wants the file saved; `None` is the user's cancel.
pub fn save_file(request: &DialogRequest) -> Result<Option<PathBuf>, DialogError> {
    platform::save_file(request)
}

/// Shows an error that happens while no window can: before the event loop runs or after it ends. Windows
/// and macOS show a message box, Linux writes `title: text` to stderr. It returns once the user closed it.
pub fn show_startup_error(title: &str, text: &str) -> io::Result<()> {
    platform::show_startup_error(title, text)
}

/// The folder the user picked; `None` is the user's cancel.
pub fn pick_folder(
    title: &str,
    parent: Option<DialogParent>,
) -> Result<Option<PathBuf>, DialogError> {
    platform::pick_folder(title, parent)
}
