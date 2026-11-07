use std::io;

use super::console_connection::ConsoleConnection;
use super::transfer::MAX_LISTING_BYTES;
use super::{AuroraRevision, ConsoleError, ConsoleStatus, Timeouts};
use crate::ftp::FtpReply;
use crate::settings::FtpSettings;

/// Servers send `421` before they close an idle connection.
const SERVICE_NOT_AVAILABLE: u16 = 421;

/// A connection to the console's FTP server, opened on demand and checked before each reuse.
pub struct ConsoleClient {
    settings: FtpSettings,
    timeouts: Timeouts,
    on_status: Box<dyn FnMut(&ConsoleStatus) + Send>,
    connection: Option<ConsoleConnection>,
    pub(super) listing_limit: u64,
}

impl ConsoleClient {
    pub fn new(
        settings: FtpSettings,
        timeouts: Timeouts,
        on_status: impl FnMut(&ConsoleStatus) + Send + 'static,
    ) -> Self {
        ConsoleClient {
            settings,
            timeouts,
            on_status: Box::new(on_status),
            connection: None,
            listing_limit: MAX_LISTING_BYTES,
        }
    }

    pub fn settings(&self) -> &FtpSettings {
        &self.settings
    }

    /// Always connects fresh, like the old `TestConnection`.
    pub fn test_connection(&mut self) -> Result<AuroraRevision, ConsoleError> {
        self.connect()
    }

    /// Keeps a live connection; reconnects when the console has dropped it.
    pub fn ensure_connected(&mut self) -> Result<(), ConsoleError> {
        if self.connection.is_none() {
            return self.connect().map(|_| ());
        }
        match self.execute("NOOP") {
            Ok(reply) if reply.code != SERVICE_NOT_AVAILABLE => Ok(()),
            // A dropped link fails the probe or answers 421, which is expected; a failed
            // reconnect is reported.
            Ok(_) | Err(ConsoleError::Protocol { .. }) => self.connect().map(|_| ()),
            Err(other) => Err(other),
        }
    }

    /// Sends one command and returns the reply whatever its code. A failure drops the connection,
    /// since the stream can no longer be trusted.
    pub(crate) fn execute(&mut self, command: &str) -> Result<FtpReply, ConsoleError> {
        let result = self
            .connection(&format!("send {command}"))?
            .execute(command);
        if result.is_err() {
            self.connection = None;
        }
        result
    }

    /// Runs a public operation on a live connection and drops the connection when it fails.
    pub(super) fn run_operation<T>(
        &mut self,
        operation: impl FnOnce(&mut ConsoleClient) -> Result<T, ConsoleError>,
    ) -> Result<T, ConsoleError> {
        let result = self.ensure_connected().and_then(|()| operation(self));
        if result.is_err() {
            self.connection = None;
        }
        result
    }

    pub(super) fn connection(
        &mut self,
        action: &str,
    ) -> Result<&mut ConsoleConnection, ConsoleError> {
        self.connection
            .as_mut()
            .ok_or_else(|| ConsoleError::Protocol {
                action: action.to_owned(),
                source: suppaftp::FtpError::ConnectionError(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "no connection to the console is open",
                )),
            })
    }

    pub(super) fn report(&mut self, status: ConsoleStatus) {
        (self.on_status)(&status);
    }

    fn connect(&mut self) -> Result<AuroraRevision, ConsoleError> {
        self.connection = None;
        let (connection, revision) =
            ConsoleConnection::open(&self.settings, self.timeouts, &mut self.on_status)?;
        self.connection = Some(connection);
        Ok(revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_client_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ConsoleClient>();
    }
}
