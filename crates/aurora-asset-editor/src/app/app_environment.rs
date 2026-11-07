use std::error::Error;
use std::path::PathBuf;

use aurora_console::FtpSettings;

use crate::about::BuildInfo;
use crate::jobs::JobRunner;
use crate::logging::ErrorLog;
use crate::platform::{Clipboard, Clock, FileDialogs, FolderOpener};
use crate::settings::AppSettings;

/// Something that went wrong before the window opened; the app logs it and shows it once started.
#[derive(Debug)]
pub struct StartupProblem {
    pub context: String,
    pub error: Box<dyn Error + Send + Sync>,
}

/// Everything the app is given at start: loaded values and the platform services.
pub struct AppEnvironment {
    pub config_dir: PathBuf,
    pub settings: AppSettings,
    /// `settings.json` exists but could not be read; it is left as it is until the user changes a setting.
    pub keep_settings_file: bool,
    pub ftp_settings: Option<FtpSettings>,
    pub subnet_prefix: String,
    pub build: BuildInfo,
    pub error_log: ErrorLog,
    pub dialogs: Box<dyn FileDialogs>,
    pub clipboard: Box<dyn Clipboard>,
    pub folder_opener: Box<dyn FolderOpener>,
    pub clock: Box<dyn Clock>,
    pub jobs: Box<dyn JobRunner>,
    pub startup_problems: Vec<StartupProblem>,
}
