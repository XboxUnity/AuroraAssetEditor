use super::window_geometry::WindowGeometry;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const APP_SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            ThemeChoice::System => system_dark,
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
        }
    }
}

/// Missing keys take their defaults, unknown keys are ignored and an unknown theme name falls back to the
/// default (with a warning), so a file from a newer version still loads. A known key holding the wrong JSON type
/// makes the file malformed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub auto_resize: bool,
    pub use_compression: bool,
    #[serde(deserialize_with = "theme_or_default")]
    pub theme: ThemeChoice,
    pub window: Option<WindowGeometry>,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            auto_resize: true,
            use_compression: true,
            theme: ThemeChoice::System,
            window: None,
        }
    }
}

fn theme_or_default<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ThemeChoice, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::String(name) => {
            let known = ThemeChoice::deserialize(value.clone());
            Ok(known.unwrap_or_else(|_| {
                log::warn!("settings key \"theme\" has unknown value {name:?}, using the default");
                ThemeChoice::default()
            }))
        }
        other => Err(D::Error::custom(format!(
            "key \"theme\" must be a string, found {other}"
        ))),
    }
}

#[derive(Debug)]
pub enum AppSettingsError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Write {
        path: PathBuf,
        source: io::Error,
    },
    Malformed {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for AppSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "could not read {}", path.display()),
            Self::Write { path, .. } => write!(f, "could not write {}", path.display()),
            Self::Malformed { path, .. } => {
                write!(f, "{} is not a valid settings file", path.display())
            }
        }
    }
}

impl std::error::Error for AppSettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Malformed { source, .. } => Some(source),
        }
    }
}

impl AppSettings {
    /// `Ok(None)` when there is no file yet.
    pub fn load(path: &Path) -> Result<Option<AppSettings>, AppSettingsError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(AppSettingsError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|source| AppSettingsError::Malformed {
                path: path.to_path_buf(),
                source,
            })
    }

    /// Writes a temp file in the same folder, then renames it over the settings file.
    pub fn save(&self, path: &Path) -> Result<(), AppSettingsError> {
        let write_error = |source| AppSettingsError::Write {
            path: path.to_path_buf(),
            source,
        };
        let folder = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder).map_err(write_error)?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|source| write_error(io::Error::other(source)))?;
        let mut temp_file = NamedTempFile::new_in(folder).map_err(write_error)?;
        temp_file.write_all(json.as_bytes()).map_err(write_error)?;
        temp_file
            .persist(path)
            .map_err(|failure| write_error(failure.error))?;
        Ok(())
    }
}
