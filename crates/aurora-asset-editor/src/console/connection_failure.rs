use aurora_console::ConsoleError;

/// Whether the error was raised while opening the connection, which the user sees as "Connection failed".
/// Every other error belongs to the operation that was running and goes to the status line.
pub fn is_connection_failure(error: &ConsoleError) -> bool {
    match error {
        ConsoleError::Settings(_)
        | ConsoleError::InvalidTimeout { .. }
        | ConsoleError::Resolve { .. }
        | ConsoleError::Connect { .. }
        | ConsoleError::LoginRejected { .. }
        | ConsoleError::TooManyConnections
        | ConsoleError::NotAurora { .. } => true,
        ConsoleError::DataConnectionFailed { .. }
        | ConsoleError::Protocol { .. }
        | ConsoleError::MissingFolder { .. }
        | ConsoleError::Refused { .. }
        | ConsoleError::ListingTooLarge { .. }
        | ConsoleError::FileTooLarge { .. }
        | ConsoleError::SizeMismatch { .. }
        | ConsoleError::InvalidAsset { .. }
        | ConsoleError::ContentDbMissing
        | ConsoleError::ContentDb(_)
        | ConsoleError::TempFile(_) => false,
    }
}
