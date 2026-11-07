use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use super::FakeConsoleConfig;
use super::file_system::FakeFileSystem;

/// Aurora's FtpDll serves at most this many sessions at once.
pub(crate) const MAX_SESSIONS: usize = 6;

/// Everything the acceptor, the sessions and the test-facing handle share.
pub(crate) struct Shared {
    pub(crate) config: FakeConsoleConfig,
    pub(crate) stopping: AtomicBool,
    pub(crate) refusing_connections: AtomicBool,
    pub(crate) timing_out_next_command: AtomicBool,
    pub(crate) active_sessions: AtomicUsize,
    pub(crate) refusing_data_connections: AtomicBool,
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
    pub(crate) stalled_listings: HashSet<String>,
    pub(crate) folder_change_reply: Option<String>,
    pub(crate) commands: Vec<String>,
    pub(crate) login_count: usize,
    pub(crate) control_sockets: Vec<TcpStream>,
    pub(crate) data_sockets: Vec<TcpStream>,
    /// Connections answered `421 Too many connections`, which Aurora never closes.
    pub(crate) refused_sockets: Vec<TcpStream>,
}

impl Shared {
    pub(crate) fn new(config: FakeConsoleConfig) -> Shared {
        Shared {
            config,
            stopping: AtomicBool::new(false),
            refusing_connections: AtomicBool::new(false),
            timing_out_next_command: AtomicBool::new(false),
            active_sessions: AtomicUsize::new(0),
            refusing_data_connections: AtomicBool::new(false),
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

/// Closes the connection for every handle to it, so the peer sees the end of the stream even
/// while a clone is still registered in the state.
pub(crate) fn shut_down(socket: &TcpStream) {
    if let Err(error) = socket.shutdown(Shutdown::Both)
        && error.kind() != io::ErrorKind::NotConnected
    {
        eprintln!("fake console: closing a connection failed: {error}");
    }
}
