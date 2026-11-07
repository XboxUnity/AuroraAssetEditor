use std::fmt;

use super::AuroraRevision;

/// Progress the client reports while it works.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleStatus {
    Connecting { host: String },
    CheckingForAurora { host: String },
    AuroraFound { revision: AuroraRevision },
    OpeningFolder { path: String },
    Downloading { file: String },
    Uploading { file: String },
}

impl fmt::Display for ConsoleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connecting { host } => write!(f, "Connecting to {host}..."),
            Self::CheckingForAurora { host } => {
                write!(f, "Connected to {host}, checking for Aurora...")
            }
            Self::AuroraFound { revision } => {
                write!(f, "Connected to Aurora revision {}", revision.as_str())
            }
            Self::OpeningFolder { path } => write!(f, "Opening {path}..."),
            Self::Downloading { file } => write!(f, "Downloading {file}..."),
            Self::Uploading { file } => write!(f, "Uploading {file}..."),
        }
    }
}
