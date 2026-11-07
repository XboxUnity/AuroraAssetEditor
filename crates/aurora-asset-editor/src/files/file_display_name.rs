use std::path::Path;

/// The file name without its folder, as dialogs and the status line name a file.
pub fn file_display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}
