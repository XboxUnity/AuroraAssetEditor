use std::path::Path;

use crate::settings::{APP_SETTINGS_FILE, AppSettings};

use super::StartupProblem;

pub const SETTINGS_LOAD_FAILED: &str = "Could not load the settings, so the defaults are in use";

/// `settings.json` as read at start, ready for `AppEnvironment`.
#[derive(Debug)]
pub struct StartupSettings {
    pub settings: AppSettings,
    /// See `AppEnvironment::keep_settings_file`.
    pub keep_file: bool,
    pub problem: Option<StartupProblem>,
}

impl StartupSettings {
    /// A missing file gives the defaults. A file that cannot be read or parsed gives the defaults and a
    /// startup problem, and is kept, so a hand-edited file is not lost to the defaults.
    pub fn load(config_dir: &Path) -> StartupSettings {
        match AppSettings::load(&config_dir.join(APP_SETTINGS_FILE)) {
            Ok(loaded) => StartupSettings {
                settings: loaded.unwrap_or_default(),
                keep_file: false,
                problem: None,
            },
            Err(error) => StartupSettings {
                settings: AppSettings::default(),
                keep_file: true,
                problem: Some(StartupProblem {
                    context: SETTINGS_LOAD_FAILED.to_owned(),
                    error: Box::new(error),
                }),
            },
        }
    }
}
