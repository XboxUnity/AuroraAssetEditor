use std::fmt;
use std::io;

use crate::settings::SettingsError;

#[derive(Debug)]
pub enum ConsoleError {
    Settings(SettingsError),
    Resolve {
        host: String,
        source: io::Error,
    },
    Connect {
        host: String,
        port: u16,
        source: io::Error,
    },
    LoginRejected {
        reply: String,
    },
    NotAurora {
        reply: String,
    },
    Protocol {
        action: String,
        source: suppaftp::FtpError,
    },
}

impl fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Settings(source) => write!(f, "the FTP settings are not usable: {source}"),
            Self::Resolve { host, .. } => write!(f, "could not find the console at {host}"),
            Self::Connect { host, port, .. } => {
                write!(f, "could not connect to {host} on port {port}")
            }
            Self::LoginRejected { reply } => write!(f, "the console rejected the login: {reply}"),
            Self::NotAurora { reply } => {
                write!(f, "the console does not look like it runs Aurora: {reply}")
            }
            Self::Protocol { action, .. } => write!(f, "could not {action}"),
        }
    }
}

impl std::error::Error for ConsoleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Settings(source) => Some(source),
            Self::Resolve { source, .. } | Self::Connect { source, .. } => Some(source),
            Self::Protocol { source, .. } => Some(source),
            Self::LoginRejected { .. } | Self::NotAurora { .. } => None,
        }
    }
}

impl From<SettingsError> for ConsoleError {
    fn from(error: SettingsError) -> Self {
        ConsoleError::Settings(error)
    }
}
