use std::io;
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::FakeConsoleConfig;
use super::console_state::{Shared, shut_down};
use super::file_system::{path_key, resolve_path};
use super::session;

const ACCEPT_POLL: Duration = Duration::from_millis(5);
const AURORA_DATA_FOLDER: &str = "/Game/Data";
const GAME_DATA_FOLDER: &str = "/Game/Data/GameData";
const CONTENT_DB_PATH: &str = "/Game/Data/DataBases/Content.db";

/// An in-process FTP server on 127.0.0.1 that behaves like the Xbox 360 console. Dropping it stops
/// the listener and every session thread.
pub struct FakeConsole {
    shared: Arc<Shared>,
    port: u16,
    acceptor: Mutex<Option<JoinHandle<()>>>,
}

impl FakeConsole {
    pub fn start(config: FakeConsoleConfig) -> FakeConsole {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .expect("binding a free port on 127.0.0.1 failed");
        listener
            .set_nonblocking(true)
            .expect("the listener must support non-blocking accepts");
        let port = listener
            .local_addr()
            .expect("a bound listener has an address")
            .port();
        let shared = Arc::new(Shared::new(config));
        let acceptor = thread::spawn({
            let shared = Arc::clone(&shared);
            move || accept_sessions(&listener, &shared)
        });
        FakeConsole {
            shared,
            port,
            acceptor: Mutex::new(Some(acceptor)),
        }
    }

    /// A console with the folders and `Content.db` Aurora keeps its game data in.
    pub fn with_aurora_layout(config: FakeConsoleConfig, content_db: &[u8]) -> FakeConsole {
        let console = FakeConsole::start(config);
        console.add_dir(AURORA_DATA_FOLDER);
        console.add_dir(GAME_DATA_FOLDER);
        console.add_file(CONTENT_DB_PATH, content_db);
        console
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn add_dir(&self, path: &str) {
        self.shared
            .state()
            .file_system
            .add_folder(&resolve_path(&[], path));
    }

    pub fn add_file(&self, path: &str, bytes: &[u8]) {
        self.shared
            .state()
            .file_system
            .put_file(&resolve_path(&[], path), bytes);
    }

    pub fn file(&self, path: &str) -> Option<Vec<u8>> {
        let state = self.shared.state();
        state
            .file_system
            .file(&resolve_path(&[], path))
            .map(<[u8]>::to_vec)
    }

    /// Listings report `size` for the file, whatever its real length.
    pub fn list_size_as(&self, path: &str, size: u64) {
        let key = path_key(&resolve_path(&[], path));
        self.shared.state().listed_sizes.insert(key, size);
    }

    /// `RETR` of the file sends `keep` bytes, closes the data connection and answers `226`.
    pub fn truncate_download(&self, path: &str, keep: usize) {
        let key = path_key(&resolve_path(&[], path));
        self.shared.state().truncated_downloads.insert(key, keep);
    }

    /// `RETR` of the file sends `after` bytes, then goes silent without closing.
    pub fn stall_download(&self, path: &str, after: usize) {
        let key = path_key(&resolve_path(&[], path));
        self.shared.state().stalled_downloads.insert(key, after);
    }

    /// Closes every control connection that is open now.
    pub fn drop_connections(&self) {
        let sockets = std::mem::take(&mut self.shared.state().control_sockets);
        for socket in sockets {
            shut_down(&socket);
        }
    }

    /// Closes the listener, so later connects fail; open sessions go on.
    pub fn refuse_new_connections(&self) {
        self.shared
            .refusing_connections
            .store(true, Ordering::SeqCst);
        self.join_acceptor();
    }

    /// Every command received, in order, with the password masked.
    pub fn commands(&self) -> Vec<String> {
        self.shared.state().commands.clone()
    }

    pub fn login_count(&self) -> usize {
        self.shared.state().login_count
    }

    fn join_acceptor(&self) {
        let acceptor = self
            .acceptor
            .lock()
            .expect("the acceptor handle lock is never poisoned")
            .take();
        if let Some(acceptor) = acceptor {
            acceptor.join().expect("the fake console acceptor panicked");
        }
    }
}

impl Drop for FakeConsole {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        self.join_acceptor();
        self.drop_connections();
        let data_sockets = std::mem::take(&mut self.shared.state().data_sockets);
        for socket in data_sockets {
            shut_down(&socket);
        }
        let sessions = std::mem::take(
            &mut *self
                .shared
                .session_threads
                .lock()
                .expect("the session list lock is never poisoned"),
        );
        for session in sessions {
            if session.join().is_err() && !thread::panicking() {
                panic!("a fake console session panicked");
            }
        }
    }
}

fn accept_sessions(listener: &TcpListener, shared: &Arc<Shared>) {
    while !shared.stopping.load(Ordering::SeqCst)
        && !shared.refusing_connections.load(Ordering::SeqCst)
    {
        match listener.accept() {
            Ok((stream, _)) => start_session(stream, shared),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(error) => {
                eprintln!("fake console: accepting a connection failed: {error}");
                return;
            }
        }
    }
}

fn start_session(stream: std::net::TcpStream, shared: &Arc<Shared>) {
    let registered = stream
        .set_nonblocking(false)
        .and_then(|()| stream.try_clone());
    match registered {
        Ok(control_socket) => {
            shared.state().control_sockets.push(control_socket);
            let handle = thread::spawn({
                let shared = Arc::clone(shared);
                move || session::serve(&shared, stream)
            });
            shared
                .session_threads
                .lock()
                .expect("the session list lock is never poisoned")
                .push(handle);
        }
        Err(error) => eprintln!("fake console: preparing a connection failed: {error}"),
    }
}
