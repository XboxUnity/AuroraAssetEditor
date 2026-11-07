use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long to wait for a connection to open, and for the console to answer once it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    pub connect: Duration,
    pub read: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            connect: DEFAULT_TIMEOUT,
            read: DEFAULT_TIMEOUT,
        }
    }
}
