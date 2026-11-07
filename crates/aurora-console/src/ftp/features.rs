//! Capabilities advertised in a FEAT reply (RFC 2389), reduced to the two the client acts on.

use super::ftp_reply::FtpReply;

/// RFC 3659 ties MLSD to MLST, so either name means listings can use MLSD.
const MACHINE_LISTING_FEATURES: [&str; 2] = ["MLST", "MLSD"];
const UTF8_FEATURE: &str = "UTF8";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Features {
    pub(crate) mlsd: bool,
    pub(crate) utf8: bool,
}

/// Each info line names one feature; the reply code is the caller's concern.
pub(crate) fn features_from_reply(reply: &FtpReply) -> Features {
    let mut features = Features::default();
    for line in &reply.info_lines {
        let feature = line.trim();
        if MACHINE_LISTING_FEATURES
            .iter()
            .any(|name| starts_with_ignoring_ascii_case(feature, name))
        {
            features.mlsd = true;
        } else if starts_with_ignoring_ascii_case(feature, UTF8_FEATURE) {
            features.utf8 = true;
        }
    }
    features
}

fn starts_with_ignoring_ascii_case(text: &str, prefix: &str) -> bool {
    text.as_bytes()
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features_of(body: &str) -> Features {
        features_from_reply(&FtpReply::from_body(body.as_bytes()).unwrap())
    }

    #[test]
    fn features_from_reply_rules() {
        assert_eq!(
            features_of("211-Features:\r\n MLSD\r\n UTF8\r\n211 End\r\n"),
            Features {
                mlsd: true,
                utf8: true
            }
        );
        assert_eq!(
            features_of("211-Features:\r\n mlst type*;size*;\r\n211 End\r\n"),
            Features {
                mlsd: true,
                utf8: false
            }
        );
        assert_eq!(
            features_of("211-Features:\r\n SIZE\r\n211 End\r\n"),
            Features::default()
        );
        assert_eq!(features_of("500 Unknown command\r\n"), Features::default());
    }
}

#[cfg(test)]
mod aurora_tests {
    use super::*;

    #[test]
    fn aurora_feat_reply_advertises_utf8_only() {
        let reply = FtpReply::from_body(
            b"211-Extensions supported:\r\n XCRC filename\r\n CDUP\r\n UTF8\r\n211 END\r\n",
        )
        .unwrap();
        assert_eq!(reply.code, 211);
        assert_eq!(reply.message, "END");
        assert_eq!(
            features_from_reply(&reply),
            Features {
                mlsd: false,
                utf8: true
            }
        );
    }

    #[test]
    fn features_only_turn_on_and_ignore_the_final_line() {
        let reply =
            FtpReply::from_body(b"211-Features:\r\n utf8\r\n SIZE\r\n\tMLSD\r\n211 MLSD UTF8\r\n")
                .unwrap();
        assert_eq!(
            features_from_reply(&reply),
            Features {
                mlsd: true,
                utf8: true
            }
        );
        let reply = FtpReply::from_body(b"211-Features:\r\n SIZE\r\n211 MLSD UTF8\r\n").unwrap();
        assert_eq!(features_from_reply(&reply), Features::default());
    }
}
