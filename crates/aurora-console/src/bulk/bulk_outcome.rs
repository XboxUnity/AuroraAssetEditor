use std::fmt;

use aurora_asset::EncodeError;

use crate::client::ConsoleError;

#[derive(Debug)]
pub enum BulkOutcome {
    Uploaded,
    AlreadyHasCover,
    /// The console's cover file can't be parsed, so it is left alone.
    ExistingCoverUnreadable(ConsoleError),
    NoCoverFound,
    Failed(BulkFailure),
}

/// Why one game's cover could not be written; the run goes on with the next game.
#[derive(Debug)]
pub enum BulkFailure {
    Console(ConsoleError),
    Provider(Box<dyn std::error::Error + Send + Sync>),
    Encode(EncodeError),
}

impl fmt::Display for BulkFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Console(_) => write!(f, "the console could not be read or written"),
            Self::Provider(_) => write!(f, "the cover could not be fetched"),
            Self::Encode(_) => write!(f, "the cover could not be encoded"),
        }
    }
}

impl std::error::Error for BulkFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Console(source) => Some(source),
            Self::Provider(source) => Some(source.as_ref()),
            Self::Encode(source) => Some(source),
        }
    }
}
