const FILE_SCHEME: &str = "file://";
const LOCAL_HOST: &str = "localhost";
const PERCENT: u8 = b'%';
const HEX_RADIX: u32 = 16;

/// The path bytes of a local `file://` URI, percent-decoded. A URI naming another host is not a local file.
pub fn file_uri_path_bytes(uri: &str) -> Option<Vec<u8>> {
    let after_scheme = uri.strip_prefix(FILE_SCHEME)?;
    let path_start = after_scheme.find('/')?;
    let host = &after_scheme[..path_start];
    if !host.is_empty() && host != LOCAL_HOST {
        return None;
    }
    percent_decode(&after_scheme[path_start..])
}

/// `None` when a `%` is not followed by two hex digits.
pub fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == PERCENT {
            let high = char::from(bytes.next()?).to_digit(HEX_RADIX)?;
            let low = char::from(bytes.next()?).to_digit(HEX_RADIX)?;
            decoded.push(u8::try_from(high * HEX_RADIX + low).ok()?);
        } else {
            decoded.push(byte);
        }
    }
    Some(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_handles_escapes() {
        assert_eq!(percent_decode("a%FFb"), Some(vec![b'a', 0xFF, b'b']));
        assert_eq!(percent_decode("a%2Fb"), Some(b"a/b".to_vec()));
        assert_eq!(percent_decode("%G1"), None);
        assert_eq!(percent_decode("%4"), None);
    }

    #[test]
    fn only_local_hosts_are_files() {
        assert_eq!(
            file_uri_path_bytes("file:///home/a%20b"),
            Some(b"/home/a b".to_vec())
        );
        assert_eq!(
            file_uri_path_bytes("file://localhost/tmp/x"),
            Some(b"/tmp/x".to_vec())
        );
        assert_eq!(file_uri_path_bytes("file://server/share/x"), None);
        assert_eq!(file_uri_path_bytes("https://example.com/x"), None);
    }
}
