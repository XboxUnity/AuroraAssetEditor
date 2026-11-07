use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};

pub const FTP_SETTINGS_FILE: &str = "ftp.json";

const APP_FOLDER_NAME: &str = "AuroraAssetEditor";
const DEFAULT_PORT: u16 = 21;
const DEFAULT_CREDENTIAL: &str = "xboxftp";
const UTF8_BOM: char = '\u{feff}';
// TEST-NET-1: never routed, and connecting a UDP socket sends nothing.
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

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

    /// Follows the old `int.TryParse` (NumberStyles.Integer): text that is not an `i32` falls back to 21.
    pub fn port_number(&self) -> Result<u16, SettingsError> {
        let number = self
            .port
            .trim_end_matches('\0')
            .trim_matches(is_dotnet_whitespace);
        let Ok(port) = number.parse::<i32>() else {
            return Ok(DEFAULT_PORT);
        };
        match u16::try_from(port) {
            Ok(port) if port != 0 => Ok(port),
            _ => Err(SettingsError::PortOutOfRange { port }),
        }
    }
}

/// .NET's `NumberStyles.Integer` white space: U+0009 to U+000D and U+0020 only.
fn is_dotnet_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ')
}

pub fn app_config_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|base| config_dir_in(base.config_dir()))
}

pub(crate) fn config_dir_in(config_home: &Path) -> PathBuf {
    config_home.join(APP_FOLDER_NAME)
}

pub fn local_subnet_prefix() -> io::Result<String> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(ROUTE_PROBE_ADDRESS)?;
    prefix_of_local_address(socket.local_addr()?.ip())
}

pub(crate) fn prefix_of_local_address(address: IpAddr) -> io::Result<String> {
    match address {
        IpAddr::V4(v4) if !v4.is_unspecified() => Ok(subnet_prefix(v4)),
        _ => Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("no usable IPv4 address on the default route (got {address})"),
        )),
    }
}

pub(crate) fn subnet_prefix(address: Ipv4Addr) -> String {
    let [a, b, c, _] = address.octets();
    format!("{a}.{b}.{c}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_joins_app_folder() {
        let home = Path::new("C:\\Users\\x\\AppData\\Roaming");
        assert_eq!(config_dir_in(home), home.join("AuroraAssetEditor"));
    }

    #[test]
    fn subnet_prefix_keeps_three_octets() {
        assert_eq!(subnet_prefix(Ipv4Addr::new(192, 168, 1, 50)), "192.168.1.");
    }

    #[test]
    fn unusable_local_address_is_error() {
        assert!(prefix_of_local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED)).is_err());
        assert!(prefix_of_local_address(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)).is_err());
        assert_eq!(
            prefix_of_local_address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7))).unwrap(),
            "10.0.0."
        );
    }
}
