use std::fmt;

#[derive(Debug)]
pub enum OnlineError {
    /// The request did not complete: no connection, a timeout, or a body cut short.
    Request {
        url: String,
        source: Box<ureq::Error>,
    },
    HttpStatus {
        url: String,
        status: u16,
    },
    InvalidResponse {
        url: String,
        reason: String,
    },
    EmptyImage {
        url: String,
    },
    KeywordsNeedXboxUnity,
}

impl fmt::Display for OnlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request { url, .. } => write!(f, "requesting {url} failed"),
            Self::HttpStatus { url, status } => write!(f, "{url} answered HTTP {status}"),
            Self::InvalidResponse { url, reason } => {
                write!(f, "the answer from {url} is not usable: {reason}")
            }
            Self::EmptyImage { url } => write!(f, "{url} sent an empty image"),
            Self::KeywordsNeedXboxUnity => {
                f.write_str("the Internet Archive can only be searched by TitleID")
            }
        }
    }
}

impl std::error::Error for OnlineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
