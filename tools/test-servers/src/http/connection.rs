use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};

use super::recorded_http::Shared;
use super::route::{Route, RouteBehaviour};

const REDIRECT_STATUS: u16 = 302;
const NOT_FOUND_STATUS: u16 = 404;
const DRAIN_BUFFER_BYTES: usize = 64;

pub(super) fn serve(shared: &Shared, stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let Some(path) = read_request_path(&mut reader) else {
        return;
    };
    shared.record_request(&path);
    match shared.route_for(&path) {
        Some(route) => answer(&stream, &mut reader, route),
        None => {
            shared.record_unmatched(&path);
            send(&stream, NOT_FOUND_STATUS, "text/plain", &[], b"no route");
        }
    }
}

/// The request target of the first line, once the blank line ending the head has been read.
fn read_request_path(reader: &mut BufReader<&TcpStream>) -> Option<String> {
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        if header.trim_end().is_empty() {
            break;
        }
    }
    request_line.split_whitespace().nth(1).map(str::to_owned)
}

fn answer(stream: &TcpStream, reader: &mut BufReader<&TcpStream>, route: &Route) {
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
        RouteBehaviour::NeverRespond => wait_for_peer_to_leave(reader),
    }
}

fn wait_for_peer_to_leave(reader: &mut BufReader<&TcpStream>) {
    let mut sink = [0u8; DRAIN_BUFFER_BYTES];
    while matches!(reader.read(&mut sink), Ok(read) if read > 0) {}
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
