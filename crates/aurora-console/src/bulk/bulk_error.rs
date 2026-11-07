use std::fmt;

use crate::client::ConsoleError;

/// The run could not start; once it has started, problems are recorded in the report instead.
#[derive(Debug)]
pub enum BulkError {
    NoGames,
    Connection(ConsoleError),
}

impl fmt::Display for BulkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGames => write!(f, "there are no games to fetch covers for"),
            Self::Connection(_) => {
                write!(f, "the connection to the console could not be established")
            }
        }
    }
}

impl std::error::Error for BulkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoGames => None,
            Self::Connection(source) => Some(source),
        }
    }
}
