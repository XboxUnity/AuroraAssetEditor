use dotnet_number_text::trim_number_text;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub const FTP_SETTINGS_FILE: &str = "ftp.json";

const DEFAULT_PORT: u16 = 21;
const DEFAULT_CREDENTIAL: &str = "xboxftp";
const UTF8_BOM: char = '\u{feff}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtpSettings {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
}

impl Default for FtpSettings {
    fn default() -> Self {
        FtpSettings {
            host: String::new(),
            port: DEFAULT_PORT.to_string(),
            username: DEFAULT_CREDENTIAL.to_string(),
            password: DEFAULT_CREDENTIAL.to_string(),
        }
    }
}

/// The old app's `DataContractJsonSerializer` shape: all strings, keys in alphabetical order.
#[derive(Serialize, Deserialize)]
struct FtpSettingsFile {
    ip: Option<String>,
    pass: Option<String>,
    port: Option<String>,
    user: Option<String>,
}

#[derive(Debug)]
pub enum SettingsError {
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
    PortOutOfRange {
        port: i32,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "could not read {}", path.display()),
            Self::Write { path, .. } => write!(f, "could not write {}", path.display()),
            Self::Malformed { path, .. } => {
                write!(f, "{} is not a valid FTP settings file", path.display())
            }
            Self::PortOutOfRange { port } => {
                write!(f, "port {port} is outside the range 1 to {}", u16::MAX)
            }
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Malformed { source, .. } => Some(source),
            Self::PortOutOfRange { .. } => None,
        }
    }
}

impl FtpSettings {
    pub fn load(path: &Path) -> Result<Option<FtpSettings>, SettingsError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SettingsError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let json = text.strip_prefix(UTF8_BOM).unwrap_or(&text);
        let file: FtpSettingsFile =
            serde_json::from_str(json).map_err(|source| SettingsError::Malformed {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Some(FtpSettings {
            host: file.ip.unwrap_or_default(),
            port: file.port.unwrap_or_default(),
            username: file.user.unwrap_or_default(),
            password: file.pass.unwrap_or_default(),
        }))
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let write_error = |source| SettingsError::Write {
            path: path.to_path_buf(),
            source,
        };
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(write_error)?;
        }
        let file = FtpSettingsFile {
            ip: Some(self.host.clone()),
            pass: Some(self.password.clone()),
            port: Some(self.port.clone()),
            user: Some(self.username.clone()),
        };
        let json =
            serde_json::to_string(&file).map_err(|source| write_error(io::Error::other(source)))?;
        std::fs::write(path, json).map_err(write_error)
    }

    /// Follows the old `int.TryParse` (NumberStyles.Integer): text that is not an `i32`, and port 0 (netftp's `Port` getter), fall back to 21.
    pub fn port_number(&self) -> Result<u16, SettingsError> {
        let Ok(port) = trim_number_text(&self.port).parse::<i32>() else {
            return Ok(DEFAULT_PORT);
        };
        match u16::try_from(port) {
            Ok(0) => Ok(DEFAULT_PORT),
            Ok(port) => Ok(port),
            _ => Err(SettingsError::PortOutOfRange { port }),
        }
    }
}
