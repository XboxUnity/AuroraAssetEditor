use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use super::FakeConsoleConfig;
use super::file_system::FakeFileSystem;

/// Everything the acceptor, the sessions and the test-facing handle share.
pub(crate) struct Shared {
    pub(crate) config: FakeConsoleConfig,
    pub(crate) stopping: AtomicBool,
    pub(crate) refusing_connections: AtomicBool,
    pub(crate) session_threads: Mutex<Vec<JoinHandle<()>>>,
    state: Mutex<ConsoleState>,
}

#[derive(Default)]
pub(crate) struct ConsoleState {
    pub(crate) file_system: FakeFileSystem,
    /// Faults, keyed by `path_key`.
    pub(crate) listed_sizes: HashMap<String, u64>,
    pub(crate) truncated_downloads: HashMap<String, usize>,
    pub(crate) stalled_downloads: HashMap<String, usize>,
    pub(crate) commands: Vec<String>,
    pub(crate) login_count: usize,
    pub(crate) control_sockets: Vec<TcpStream>,
}

impl Shared {
    pub(crate) fn new(config: FakeConsoleConfig) -> Shared {
        Shared {
            config,
            stopping: AtomicBool::new(false),
            refusing_connections: AtomicBool::new(false),
            session_threads: Mutex::new(Vec::new()),
            state: Mutex::new(ConsoleState::default()),
        }
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, ConsoleState> {
        self.state
            .lock()
            .expect("a fake console thread panicked while holding the state")
    }
}
