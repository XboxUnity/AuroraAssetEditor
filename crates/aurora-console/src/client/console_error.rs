use std::fmt;
use std::io;

use aurora_asset::AssetError;

use crate::games::ContentDbError;
use crate::settings::SettingsError;

#[derive(Debug)]
pub enum ConsoleError {
    Settings(SettingsError),
    InvalidTimeout {
        timeout: &'static str,
    },
    Resolve {
        host: String,
        source: io::Error,
    },
    Connect {
        host: String,
        port: u16,
        source: io::Error,
    },
    /// Only the reply code is kept: Aurora's reply to a wrong password echoes the stored password hash.
    LoginRejected {
        code: u32,
    },
    NotAurora {
        reply: String,
    },
    Protocol {
        action: String,
        source: suppaftp::FtpError,
    },
    MissingFolder {
        path: String,
    },
    /// A 4xx reply: the console may accept the same command later.
    Refused {
        action: String,
        reply: String,
    },
    ListingTooLarge {
        path: String,
        limit: u64,
    },
    FileTooLarge {
        file: String,
        size: u64,
        limit: u64,
    },
    /// `received` stops counting one byte past `listed`, where the download is cut off.
    SizeMismatch {
        file: String,
        listed: u64,
        received: u64,
    },
    InvalidAsset {
        file: String,
        source: AssetError,
    },
    /// The console has no `Content.db`, or an empty one.
    ContentDbMissing,
    ContentDb(ContentDbError),
    TempFile(io::Error),
}

impl fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settings(_) => write!(f, "the FTP settings are not usable"),
            Self::InvalidTimeout { timeout } => {
                write!(f, "the {timeout} timeout must be longer than zero")
            }
            Self::Resolve { host, .. } => write!(f, "could not find the console at {host}"),
            Self::Connect { host, port, .. } => {
                write!(f, "could not connect to {host} on port {port}")
            }
            Self::LoginRejected { code } => write!(
                f,
                "the console rejected the login (reply {code}), check the FTP user name and password"
            ),
            Self::NotAurora { reply } => {
                write!(f, "the console does not look like it runs Aurora: {reply}")
            }
            Self::Protocol { action, .. } => write!(f, "could not {action}"),
            Self::MissingFolder { path } => write!(f, "the console has no folder {path}"),
            Self::Refused { action, reply } => {
                write!(f, "the console refused to {action}: {reply}")
            }
            Self::ListingTooLarge { path, limit } => write!(
                f,
                "the listing of {path} is longer than the {limit} bytes the editor accepts"
            ),
            Self::FileTooLarge { file, size, limit } => write!(
                f,
                "{file} is listed as {size} bytes, more than the {limit} bytes the editor accepts"
            ),
            Self::SizeMismatch {
                file,
                listed,
                received,
            } => {
                if received > listed {
                    write!(
                        f,
                        "{file} is listed as {listed} bytes but the console sent more than that"
                    )
                } else {
                    write!(
                        f,
                        "{file} is listed as {listed} bytes but the console sent {received}"
                    )
                }
            }
            Self::InvalidAsset { file, .. } => write!(f, "{file} is not a valid asset file"),
            Self::ContentDbMissing => write!(f, "the console has no Content.db with any content"),
            Self::ContentDb(_) => write!(f, "the console's Content.db could not be read"),
            Self::TempFile(_) => write!(f, "could not save Content.db to a temporary file"),
        }
    }
}

impl std::error::Error for ConsoleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Settings(source) => Some(source),
            Self::Resolve { source, .. } | Self::Connect { source, .. } => Some(source),
            Self::Protocol { source, .. } => Some(source),
            Self::InvalidAsset { source, .. } => Some(source),
            Self::ContentDb(source) => Some(source),
            Self::TempFile(source) => Some(source),
            Self::InvalidTimeout { .. }
            | Self::LoginRejected { .. }
            | Self::NotAurora { .. }
            | Self::MissingFolder { .. }
            | Self::Refused { .. }
            | Self::ListingTooLarge { .. }
            | Self::FileTooLarge { .. }
            | Self::SizeMismatch { .. }
            | Self::ContentDbMissing => None,
        }
    }
}

impl From<SettingsError> for ConsoleError {
    fn from(error: SettingsError) -> Self {
        ConsoleError::Settings(error)
    }
}
