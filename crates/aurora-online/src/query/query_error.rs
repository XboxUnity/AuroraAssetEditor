use std::fmt;

use super::title_id::TITLE_ID_DIGITS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    Empty,
    NotHex { text: String },
    TooLong { len: usize },
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("enter something to search for"),
            Self::NotHex { text } => write!(f, "\"{text}\" is not made of hex digits"),
            Self::TooLong { len } => write!(
                f,
                "a TitleID has at most {TITLE_ID_DIGITS} hex digits, not {len}"
            ),
        }
    }
}

impl std::error::Error for QueryError {}
