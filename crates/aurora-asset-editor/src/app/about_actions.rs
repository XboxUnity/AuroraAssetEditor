use crate::dialogs::Modal;
use crate::status::StatusLine;

use super::App;

pub const ABOUT_TITLE: &str = "About Aurora Asset Editor";
pub const COPY_BUILD_INFO_FAILED: &str = "Could not copy the build info";
pub const OPEN_LOG_FOLDER_FAILED: &str = "Could not open the log folder";
const BUILD_INFO_COPIED_TEXT: &str = "Copied the build info";

impl App {
    pub fn show_about(&mut self) {
        self.open_menu = None;
        self.modals.push(Modal::About);
    }

    /// The About dialog's "Copy build info".
    pub fn copy_build_info(&mut self) {
        let text = self.environment.build.copy_text();
        match self.environment.clipboard.set_text(&text) {
            Ok(()) => self.status = StatusLine::info(BUILD_INFO_COPIED_TEXT.to_owned()),
            Err(error) => {
                let message = format!("{COPY_BUILD_INFO_FAILED}: {error}");
                self.fail_over_about(COPY_BUILD_INFO_FAILED, message, &error);
            }
        }
    }

    /// The About dialog's "Open log folder": the folder that holds `error.log`.
    pub fn open_log_folder(&mut self) {
        let folder = self.environment.config_dir.clone();
        if let Err(error) = self.environment.folder_opener.open_folder(&folder) {
            let message = format!("{OPEN_LOG_FOLDER_FAILED}: {error}");
            self.fail_over_about(OPEN_LOG_FOLDER_FAILED, message, &error);
        }
    }

    /// The About dialog covers the status line, so it closes to let the error dialog show.
    fn fail_over_about(
        &mut self,
        title: &str,
        text: String,
        error: &(dyn std::error::Error + 'static),
    ) {
        if matches!(self.modals.current(), Some(Modal::About)) {
            self.modals.close_current();
        }
        self.show_error(title, text, error);
    }
}
