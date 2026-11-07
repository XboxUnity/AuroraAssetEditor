use std::time::Duration;

use super::ConsoleError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long to wait for a connection to open, and for the console to answer once it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub connect: Duration,
    pub read: Duration,
}

impl Timeouts {
    /// A zero timeout is rejected by the sockets, so it is reported by name before connecting.
    pub(super) fn check(&self) -> Result<(), ConsoleError> {
        for (timeout, duration) in [("connect", self.connect), ("read", self.read)] {
            if duration.is_zero() {
                return Err(ConsoleError::InvalidTimeout { timeout });
            }
        }
        Ok(())
    }
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            connect: DEFAULT_TIMEOUT,
            read: DEFAULT_TIMEOUT,
        }
    }
}
