use aurora_asset::Compression;

use crate::settings::{APP_SETTINGS_FILE, AppSettings, ThemeChoice, WindowGeometry};

use super::App;

pub const SETTINGS_SAVE_FAILED: &str = "Could not save the settings";

impl App {
    pub fn settings(&self) -> &AppSettings {
        &self.environment.settings
    }

    pub fn set_auto_resize(&mut self, on: bool) {
        self.change_settings(|settings| settings.auto_resize = on);
    }

    pub fn set_use_compression(&mut self, on: bool) {
        self.change_settings(|settings| settings.use_compression = on);
    }

    pub fn set_theme(&mut self, theme: ThemeChoice) {
        self.change_settings(|settings| settings.theme = theme);
    }

    /// "Use Compression" as the encoder takes it; read when an image is loaded.
    pub(super) fn compression(&self) -> Compression {
        if self.environment.settings.use_compression {
            Compression::Dxt
        } else {
            Compression::None
        }
    }

    /// Stores where the window was when it closed. Moving the window is not a change of a setting, so an
    /// unreadable settings file stays as it is.
    pub fn save_window_geometry(&mut self, geometry: WindowGeometry) {
        self.environment.settings.window = Some(geometry);
        if !self.environment.keep_settings_file {
            self.save_settings();
        }
    }

    fn change_settings(&mut self, change: impl FnOnce(&mut AppSettings)) {
        change(&mut self.environment.settings);
        self.environment.keep_settings_file = false;
        self.save_settings();
    }

    fn save_settings(&mut self) {
        let path = self.environment.config_dir.join(APP_SETTINGS_FILE);
        if let Err(error) = self.environment.settings.save(&path) {
            self.report_error(SETTINGS_SAVE_FAILED, &error);
        }
    }
}
