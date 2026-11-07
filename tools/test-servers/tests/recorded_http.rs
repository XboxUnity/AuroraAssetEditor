use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use test_servers::{RecordedHttp, Route, RouteBehaviour};

struct RawResponse {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl RawResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (header_name, value) = line.split_once(':')?;
            header_name.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/online/recordings.toml")
}

fn get(server: &RecordedHttp, path: &str) -> RawResponse {
    let address = server.base_url().replace("http://", "");
    let mut stream = TcpStream::connect(&address).expect("connecting to the server");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("sending the request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("reading to the close");
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("the response has a head");
    let head = String::from_utf8(raw[..split].to_vec()).expect("the head is text");
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("the status line has a code");
    RawResponse {
        status,
        head,
        body: raw[split + 4..].to_vec(),
    }
}

fn route(path: &str, body: &[u8], behaviour: RouteBehaviour) -> Route {
    Route {
        path: path.to_string(),
        status: 200,
        content_type: "text/plain".to_string(),
        body: body.to_vec(),
        behaviour,
    }
}

#[test]
fn serves_recordings_by_path() {
    let server = RecordedHttp::from_recordings(&manifest_path(), Vec::new());
    let recorded = [
        (
            "/api/Covers/4D5307E6",
            200,
            "application/json",
            "xboxunity_title_4D5307E6.json",
        ),
        (
            "/api/Covers/FFFFFFFF",
            200,
            "application/json",
            "xboxunity_title_FFFFFFFF.json",
        ),
        (
            "/api/boxartsm/225",
            200,
            "image/jpeg",
            "xboxunity_thumbnail_225.jpg",
        ),
        (
            "/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/FFFFFFFF/",
            404,
            "text/html; charset=UTF-8",
            "archive_listing_FFFFFFFF.html",
        ),
    ];
    for (path, status, content_type, file) in recorded {
        let response = get(&server, path);

        assert_eq!(response.status, status, "{path}");
        assert_eq!(
            response.header("Content-Type"),
            Some(content_type),
            "{path}"
        );
        let expected = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/online/recorded")
                .join(file),
        )
        .expect("reading the recorded body");
        assert_eq!(response.body, expected, "{path}");
    }

    let paths: Vec<&str> = recorded.iter().map(|entry| entry.0).collect();
    assert_eq!(server.requests(), paths);
    assert!(server.unmatched().is_empty());
}

#[test]
fn substitutes_server_placeholder() {
    let server = RecordedHttp::start(vec![route("/page", b"{server}/x", RouteBehaviour::Normal)]);

    let response = get(&server, "/page");

    assert_eq!(
        response.body,
        format!("{}/x", server.base_url()).into_bytes()
    );
}

#[test]
fn truncated_route_closes_early() {
    let body = b"0123456789";
    let server = RecordedHttp::start(vec![route(
        "/cut",
        body,
        RouteBehaviour::Truncated { declared_len: 1000 },
    )]);

    let response = get(&server, "/cut");

    assert_eq!(response.header("Content-Length"), Some("1000"));
    assert_eq!(response.body, body);
}

#[test]
fn redirect_route_sends_location() {
    let server = RecordedHttp::start(vec![route(
        "/old",
        b"",
        RouteBehaviour::Redirect {
            location: "{server}/target".to_string(),
        },
    )]);

    let response = get(&server, "/old");

    assert_eq!(response.status, 302);
    assert_eq!(
        response.header("Location"),
        Some(format!("{}/target", server.base_url()).as_str())
    );
}

#[test]
fn unknown_path_is_404_and_listed() {
    let server = RecordedHttp::start(Vec::new());

    let response = get(&server, "/nowhere");

    assert_eq!(response.status, 404);
    assert_eq!(server.unmatched(), ["/nowhere"]);
}

#[test]
fn drop_stops_connections_whose_client_is_still_open() {
    const STOP_WITHIN: Duration = Duration::from_secs(3);
    let server = RecordedHttp::start(vec![route("/hang", b"", RouteBehaviour::NeverRespond)]);
    let address = server.base_url().replace("http://", "");
    let mut silent = TcpStream::connect(&address).expect("connecting without a request");
    let mut waiting = TcpStream::connect(&address).expect("connecting to the hanging route");
    write!(
        waiting,
        "GET /hang HTTP/1.1
Host: {address}

"
    )
    .expect("sending the request");
    while server.requests().is_empty() {
        thread::sleep(Duration::from_millis(5));
    }
    let (stopped, stopped_signal) = mpsc::channel();
    thread::spawn(move || {
        drop(server);
        stopped.send(()).expect("the test is still waiting");
    });

    let result = stopped_signal.recv_timeout(STOP_WITHIN);

    assert!(result.is_ok(), "dropping the server hung on an open client");
    silent.flush().expect("the silent client is still open");
}
