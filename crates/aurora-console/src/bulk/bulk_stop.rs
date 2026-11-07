use std::fmt;

use crate::client::ConsoleError;

/// Why a bulk run ended before every game was handled.
#[derive(Debug)]
pub enum BulkStop {
    /// `on_progress` returned `Break`.
    Cancelled,
    /// Reconnecting to the console failed.
    ConsoleGone(ConsoleError),
}

impl fmt::Display for BulkStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "the run was stopped"),
            Self::ConsoleGone(_) => write!(f, "the console could not be reached again"),
        }
    }
}

impl std::error::Error for BulkStop {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cancelled => None,
            Self::ConsoleGone(source) => Some(source),
        }
    }
}
