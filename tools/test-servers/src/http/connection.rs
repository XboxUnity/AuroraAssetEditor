use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use super::recorded_http::Shared;
use super::route::{Route, RouteBehaviour};

const REDIRECT_STATUS: u16 = 302;
const NOT_FOUND_STATUS: u16 = 404;
const DRAIN_BUFFER_BYTES: usize = 64;
/// A blocked read is not woken by `shutdown` on Windows, so reads time out and check the stop flag.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

pub(super) fn serve(shared: &Shared, stream: TcpStream) {
    if let Err(error) = stream.set_read_timeout(Some(POLL_INTERVAL)) {
        eprintln!("recorded http: preparing a connection failed: {error}");
        return;
    }
    let mut reader = BufReader::new(&stream);
    let Some(path) = read_request_path(shared, &mut reader) else {
        return;
    };
    shared.record_request(&path);
    match shared.route_for(&path) {
        Some(route) => answer(shared, &stream, &mut reader, route),
        None => {
            shared.record_unmatched(&path);
            send(&stream, NOT_FOUND_STATUS, "text/plain", &[], b"no route");
        }
    }
}

/// The request target of the first line, once the blank line ending the head has been read.
fn read_request_path(shared: &Shared, reader: &mut BufReader<&TcpStream>) -> Option<String> {
    let mut request_line = String::new();
    if read_line_until_stopped(shared, reader, &mut request_line).ok()? == 0 {
        return None;
    }
    loop {
        let mut header = String::new();
        if read_line_until_stopped(shared, reader, &mut header).ok()? == 0 {
            return None;
        }
        if header.trim_end().is_empty() {
            break;
        }
    }
    request_line.split_whitespace().nth(1).map(str::to_owned)
}

/// Like `read_line`, but returns 0 (as for a closed peer) once the server is stopping. Bytes read
/// before a timeout stay in `line`.
fn read_line_until_stopped(
    shared: &Shared,
    reader: &mut BufReader<&TcpStream>,
    line: &mut String,
) -> io::Result<usize> {
    loop {
        match reader.read_line(line) {
            Err(error) if is_read_timeout(&error) => {
                if shared.is_stopping() {
                    return Ok(0);
                }
            }
            result => return result,
        }
    }
}

fn is_read_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

fn answer(shared: &Shared, stream: &TcpStream, reader: &mut BufReader<&TcpStream>, route: &Route) {
    match &route.behaviour {
        RouteBehaviour::Normal => send(stream, route.status, &route.content_type, &[], &route.body),
        RouteBehaviour::Truncated { declared_len } => send_with_length(
            stream,
            route.status,
            &route.content_type,
            *declared_len,
            &[],
            &route.body,
        ),
        RouteBehaviour::Redirect { location } => send(
            stream,
            REDIRECT_STATUS,
            &route.content_type,
            &[("Location", location)],
            &[],
        ),
        RouteBehaviour::NeverRespond => wait_for_peer_to_leave(shared, reader),
    }
}

fn wait_for_peer_to_leave(shared: &Shared, reader: &mut BufReader<&TcpStream>) {
    let mut sink = [0u8; DRAIN_BUFFER_BYTES];
    loop {
        match reader.read(&mut sink) {
            Ok(0) => return,
            Ok(_) => {}
            Err(error) if is_read_timeout(&error) && !shared.is_stopping() => {}
            Err(error) if is_read_timeout(&error) => return,
            Err(error) => {
                eprintln!("recorded http: waiting for the client failed: {error}");
                return;
            }
        }
    }
}

fn send(
    stream: &TcpStream,
    status: u16,
    content_type: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) {
    send_with_length(
        stream,
        status,
        content_type,
        body.len(),
        extra_headers,
        body,
    );
}

fn send_with_length(
    stream: &TcpStream,
    status: u16,
    content_type: &str,
    declared_len: usize,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) {
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {declared_len}\r\nConnection: close\r\n",
        reason_phrase(status)
    );
    for (name, value) in extra_headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut writer = stream;
    let sent = writer
        .write_all(head.as_bytes())
        .and_then(|()| writer.write_all(body))
        .and_then(|()| writer.flush())
        .and_then(|()| stream.shutdown(Shutdown::Write));
    if let Err(error) = sent {
        eprintln!("recorded http: sending a response failed: {error}");
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    }
}
