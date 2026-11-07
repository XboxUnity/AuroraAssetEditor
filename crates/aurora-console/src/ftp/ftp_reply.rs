//! Control-connection replies (RFC 959 section 4.2) and the line splitting shared with data transfers.

const CODE_LENGTH: usize = 3;
const FINAL_LINE_SEPARATOR: u8 = b' ';
const LINE_FEED: u8 = b'\n';

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FtpReply {
    pub(crate) code: u16,
    pub(crate) message: String,
    pub(crate) info_lines: Vec<String>,
}

impl FtpReply {
    /// The reply ends at the first line shaped `NNN text`; every line before it is an info line.
    pub(crate) fn from_body(body: &[u8]) -> Option<FtpReply> {
        let mut info_lines = Vec::new();
        for line in terminated_lines(body) {
            if let Some((code, message)) = split_final_line(&line) {
                return Some(FtpReply {
                    code,
                    message: message.to_owned(),
                    info_lines,
                });
            }
            info_lines.push(line);
        }
        None
    }

    /// Positive preliminary (1xx), completion (2xx) and intermediate (3xx) replies.
    pub(crate) fn is_success(&self) -> bool {
        matches!(self.code / 100, 1..=3)
    }
}

/// Lines terminated by a line feed, with every CR and LF stripped from both ends; an unterminated tail is dropped.
pub(crate) fn terminated_lines(body: &[u8]) -> impl Iterator<Item = String> {
    body.split_inclusive(|&byte| byte == LINE_FEED)
        .filter(|line| line.ends_with(&[LINE_FEED]))
        .map(|line| {
            String::from_utf8_lossy(line)
                .trim_matches(['\r', '\n'])
                .to_owned()
        })
}

/// Splits `NNN text` into its code and text. A hyphen after the code marks a continuation line instead.
fn split_final_line(line: &str) -> Option<(u16, &str)> {
    let bytes = line.as_bytes();
    if bytes.len() <= CODE_LENGTH || bytes[CODE_LENGTH] != FINAL_LINE_SEPARATOR {
        return None;
    }
    let code_digits = &bytes[..CODE_LENGTH];
    if !code_digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let code = code_digits
        .iter()
        .fold(0, |code, digit| code * 10 + u16::from(digit - b'0'));
    Some((code, &line[CODE_LENGTH + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_single_line() {
        let reply = FtpReply::from_body(b"200 1234\r\n").unwrap();
        assert_eq!(reply.code, 200);
        assert_eq!(reply.message, "1234");
        assert!(reply.info_lines.is_empty());
    }

    #[test]
    fn reply_multi_line() {
        let reply = FtpReply::from_body(b"211-Features:\r\n MLSD\r\n UTF8\r\n211 End\r\n").unwrap();
        assert_eq!(reply.code, 211);
        assert_eq!(reply.info_lines, ["211-Features:", " MLSD", " UTF8"]);
        assert_eq!(reply.message, "End");
    }

    #[test]
    fn reply_success_rule() {
        for code in [150, 226, 331] {
            let body = format!("{code} ok\r\n");
            assert!(FtpReply::from_body(body.as_bytes()).unwrap().is_success());
        }
        for code in [421, 550] {
            let body = format!("{code} no\r\n");
            assert!(!FtpReply::from_body(body.as_bytes()).unwrap().is_success());
        }
        assert_eq!(FtpReply::from_body(b"garbage"), None);
        assert_eq!(FtpReply::from_body(b"garbage\r\n"), None);
        assert_eq!(FtpReply::from_body(b"+12 x\r\n"), None);
    }

    #[test]
    fn listing_lines_lose_all_cr_and_lf_and_the_unterminated_tail() {
        let body = b"-rw-r--r-- 1 u g 5 Nov 07 12:00 x\r\r\n\r\r\n\r\n-rw-r--r-- 1 u g 6 Nov 07 12:00 tail";
        let lines: Vec<String> = terminated_lines(body).collect();
        assert_eq!(lines, ["-rw-r--r-- 1 u g 5 Nov 07 12:00 x", "", ""]);
        let entries = crate::ftp::parse_listing(&lines);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "x");
    }

    #[test]
    fn reply_drops_unterminated_line() {
        assert_eq!(FtpReply::from_body(b"200 1234"), None);
        let reply = FtpReply::from_body(b"200 first\r\n200 second").unwrap();
        assert_eq!(reply.message, "first");
    }
}

#[cfg(test)]
mod line_shape_tests {
    use super::*;

    #[test]
    fn lines_that_are_not_final_lines() {
        for body in [
            "200\r\n",
            "2000 x\r\n",
            "211-Features:\r\n",
            "20x y\r\n",
            "ééé x\r\n",
            "2é x\r\n",
        ] {
            assert_eq!(FtpReply::from_body(body.as_bytes()), None, "{body:?}");
        }
    }

    #[test]
    fn final_line_message_is_kept_verbatim() {
        let reply = FtpReply::from_body(b"010 \r\n").unwrap();
        assert_eq!(reply.code, 10);
        assert_eq!(reply.message, "");
        let reply = FtpReply::from_body(b"220  two  spaces \r\n").unwrap();
        assert_eq!(reply.message, " two  spaces ");
    }

    #[test]
    fn lines_after_the_final_line_are_ignored() {
        let reply = FtpReply::from_body(b"200 first\r\n200 second\r\n").unwrap();
        assert_eq!(reply.message, "first");
        assert!(reply.info_lines.is_empty());
    }

    #[test]
    fn success_is_the_hundreds_digit_one_to_three() {
        let reply_with_code = |code| FtpReply {
            code,
            message: String::new(),
            info_lines: Vec::new(),
        };
        for code in [100, 399] {
            assert!(reply_with_code(code).is_success(), "{code}");
        }
        for code in [0, 99, 400, 599, 600, 999] {
            assert!(!reply_with_code(code).is_success(), "{code}");
        }
    }

    #[test]
    fn invalid_utf8_becomes_the_replacement_character() {
        let reply = FtpReply::from_body(b"550 bad \xff name\r\n").unwrap();
        assert_eq!(reply.message, "bad \u{FFFD} name");
    }

    #[test]
    fn line_ends_are_trimmed_but_other_whitespace_is_kept() {
        let lines: Vec<String> = terminated_lines(b"\r\n x\r\ty\t\r\n\n").collect();
        assert_eq!(lines, ["", " x\r\ty\t", ""]);
        assert_eq!(terminated_lines(b"no line feed").count(), 0);
    }
}

#[cfg(test)]
mod aurora_tests {
    use super::*;

    #[test]
    fn aurora_site_revision_reply() {
        let reply = FtpReply::from_body(b"200 1655\r\n").unwrap();
        assert_eq!(reply.code, 200);
        assert_eq!(reply.message, "1655");
        assert!(reply.info_lines.is_empty());
        assert!(reply.is_success());
    }

    #[test]
    fn aurora_refusals_are_not_success() {
        let reply = FtpReply::from_body(b"502 Command not found\r\n").unwrap();
        assert_eq!(reply.code, 502);
        assert!(!reply.is_success());
        let reply = FtpReply::from_body(
            b"530 Login failed: input = 0123456789ABCDEF, pass = FEDCBA9876543210\r\n",
        )
        .unwrap();
        assert_eq!(
            reply.message,
            "Login failed: input = 0123456789ABCDEF, pass = FEDCBA9876543210"
        );
        assert!(!reply.is_success());
    }

    #[test]
    fn aurora_listing_body_splits_into_lines() {
        let body = b"drwxrwxrwx   1 root root             0 Jan 05 2019 4D5307E6_00000012\r\n\
                     -rw-rw-rw-   1 root root       1234567 Nov 03 14:05 GC4D5307E6.asset\r\n";
        let lines: Vec<String> = terminated_lines(body).collect();
        assert_eq!(
            lines,
            [
                "drwxrwxrwx   1 root root             0 Jan 05 2019 4D5307E6_00000012",
                "-rw-rw-rw-   1 root root       1234567 Nov 03 14:05 GC4D5307E6.asset",
            ]
        );
    }
}
