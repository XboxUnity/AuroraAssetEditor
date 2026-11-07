use std::io;

use super::console_connection::ConsoleConnection;
use super::{AuroraRevision, ConsoleError, ConsoleStatus, Timeouts};
use crate::ftp::FtpReply;
use crate::settings::FtpSettings;

/// A connection to the console's FTP server, opened on demand and checked before each reuse.
pub struct ConsoleClient {
    settings: FtpSettings,
    timeouts: Timeouts,
    on_status: Box<dyn FnMut(&ConsoleStatus) + Send>,
    connection: Option<ConsoleConnection>,
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
            Ok(_) => Ok(()),
            // A dropped link fails the probe, which is expected; a failed reconnect is reported.
            Err(ConsoleError::Protocol { .. }) => self.connect().map(|_| ()),
            Err(other) => Err(other),
        }
    }

    /// Sends one command and returns the reply whatever its code. A failure drops the connection,
    /// since the stream can no longer be trusted.
    pub(crate) fn execute(&mut self, command: &str) -> Result<FtpReply, ConsoleError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(ConsoleError::Protocol {
                action: format!("send {command}"),
                source: suppaftp::FtpError::ConnectionError(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "no connection to the console is open",
                )),
            });
        };
        let result = connection.execute(command);
        if result.is_err() {
            self.connection = None;
        }
        result
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
