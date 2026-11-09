use std::time::Duration;

use crate::online_client::DEFAULT_TIMEOUT;

const DEFAULT_MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub timeout: Duration,
    /// Whether the environment proxy variables and the Windows system proxy apply. Turn it off
    /// for servers on 127.0.0.1, which a proxy setting must not intercept.
    pub use_proxy: bool,
    /// Longest response body accepted; a longer one is a `Request` error.
    pub max_response_bytes: u64,
    /// Refuses every destination that is not loopback (IP literals and `localhost`) before any
    /// connection or name lookup, and ignores proxies. For tests, which must never reach the internet.
    pub loopback_only: bool,
}

impl Default for ClientOptions {
    fn default() -> Self {
        ClientOptions {
            timeout: DEFAULT_TIMEOUT,
            use_proxy: true,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            loopback_only: false,
        }
    }
}
