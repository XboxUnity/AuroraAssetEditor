use std::io;
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::connection;
use super::recording_manifest::routes_from_manifest;
use super::route::{Route, RouteBehaviour};

const ACCEPT_POLL: Duration = Duration::from_millis(5);
const SERVER_PLACEHOLDER: &str = "{server}";

/// An in-process HTTP/1.1 server on 127.0.0.1 that answers fixed routes. Dropping it stops the
/// listener and every connection thread.
pub struct RecordedHttp {
    shared: Arc<Shared>,
    base_url: String,
    acceptor: Option<JoinHandle<()>>,
}

pub(super) struct Shared {
    routes: Vec<Route>,
    requests: Mutex<Vec<String>>,
    unmatched: Mutex<Vec<String>>,
    stopping: AtomicBool,
    sockets: Mutex<Vec<TcpStream>>,
    connection_threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Shared {
    pub(super) fn route_for(&self, path: &str) -> Option<&Route> {
        self.routes.iter().find(|route| route.path == path)
    }

    pub(super) fn record_request(&self, path: &str) {
        lock(&self.requests).push(path.to_owned());
    }

    pub(super) fn record_unmatched(&self, path: &str) {
        lock(&self.unmatched).push(path.to_owned());
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .expect("the recorded http locks are never poisoned")
}

impl RecordedHttp {
    pub fn start(routes: Vec<Route>) -> RecordedHttp {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .expect("binding a free port on 127.0.0.1 failed");
        listener
            .set_nonblocking(true)
            .expect("the listener must support non-blocking accepts");
        let port = listener
            .local_addr()
            .expect("a bound listener has an address")
            .port();
        let base_url = format!("http://127.0.0.1:{port}");
        let shared = Arc::new(Shared {
            routes: routes
                .into_iter()
                .map(|route| substitute_server(route, &base_url))
                .collect(),
            requests: Mutex::new(Vec::new()),
            unmatched: Mutex::new(Vec::new()),
            stopping: AtomicBool::new(false),
            sockets: Mutex::new(Vec::new()),
            connection_threads: Mutex::new(Vec::new()),
        });
        let acceptor = thread::spawn({
            let shared = Arc::clone(&shared);
            move || accept_connections(&listener, &shared)
        });
        RecordedHttp {
            shared,
            base_url,
            acceptor: Some(acceptor),
        }
    }

    /// Serves every response of a `recordings.toml`, plus `extra` routes.
    pub fn from_recordings(manifest: &Path, extra: Vec<Route>) -> RecordedHttp {
        let mut routes = routes_from_manifest(manifest);
        routes.extend(extra);
        RecordedHttp::start(routes)
    }

    pub fn base_url(&self) -> String {
        self.base_url.clone()
    }

    /// Request paths in the order they arrived.
    pub fn requests(&self) -> Vec<String> {
        lock(&self.shared.requests).clone()
    }

    /// Paths answered 404 because no route matched.
    pub fn unmatched(&self) -> Vec<String> {
        lock(&self.shared.unmatched).clone()
    }
}

impl Drop for RecordedHttp {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        if let Some(acceptor) = self.acceptor.take()
            && acceptor.join().is_err()
            && !thread::panicking()
        {
            panic!("the recorded http acceptor panicked");
        }
        for socket in std::mem::take(&mut *lock(&self.shared.sockets)) {
            if let Err(error) = socket.shutdown(Shutdown::Both)
                && error.kind() != io::ErrorKind::NotConnected
            {
                eprintln!("recorded http: closing a connection failed: {error}");
            }
        }
        for handle in std::mem::take(&mut *lock(&self.shared.connection_threads)) {
            if handle.join().is_err() && !thread::panicking() {
                panic!("a recorded http connection thread panicked");
            }
        }
    }
}

fn substitute_server(mut route: Route, base_url: &str) -> Route {
    route.body = replace_bytes(
        &route.body,
        SERVER_PLACEHOLDER.as_bytes(),
        base_url.as_bytes(),
    );
    if let RouteBehaviour::Redirect { location } = &mut route.behaviour {
        *location = location.replace(SERVER_PLACEHOLDER, base_url);
    }
    route
}

fn replace_bytes(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(haystack.len());
    let mut position = 0;
    while position < haystack.len() {
        if haystack[position..].starts_with(needle) {
            result.extend_from_slice(replacement);
            position += needle.len();
        } else {
            result.push(haystack[position]);
            position += 1;
        }
    }
    result
}

fn accept_connections(listener: &TcpListener, shared: &Arc<Shared>) {
    while !shared.stopping.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => start_connection(stream, shared),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(error) => {
                eprintln!("recorded http: accepting a connection failed: {error}");
                return;
            }
        }
    }
}

fn start_connection(stream: TcpStream, shared: &Arc<Shared>) {
    let registered = stream
        .set_nonblocking(false)
        .and_then(|()| stream.try_clone());
    match registered {
        Ok(handle_for_shutdown) => {
            lock(&shared.sockets).push(handle_for_shutdown);
            let handle = thread::spawn({
                let shared = Arc::clone(shared);
                move || connection::serve(&shared, stream)
            });
            lock(&shared.connection_threads).push(handle);
        }
        Err(error) => eprintln!("recorded http: preparing a connection failed: {error}"),
    }
}
