use std::fmt;

const TITLE_ID_DIGITS: usize = 8;
const HEX_RADIX: u32 = 16;
const HEX_DIGIT_BITS: u32 = 4;

/// Not `HexId`: the old app's `uint.TryParse(.., HexNumber)` accepts surrounding whitespace and
/// leading zeros beyond eight digits, where `HexId` wants exactly eight digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitleId(u32);

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

impl TitleId {
    pub fn new(value: u32) -> TitleId {
        TitleId(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }

    pub fn parse(text: &str) -> Result<TitleId, QueryError> {
        let trimmed = trim_like_dotnet(text);
        if trimmed.is_empty() {
            return Err(QueryError::Empty);
        }
        if !trimmed.chars().all(|c| c.is_digit(HEX_RADIX)) {
            return Err(QueryError::NotHex {
                text: trimmed.to_string(),
            });
        }
        let significant = trimmed.trim_start_matches('0');
        if significant.len() > TITLE_ID_DIGITS {
            return Err(QueryError::TooLong {
                len: trimmed.chars().count(),
            });
        }
        let value = if significant.is_empty() {
            0
        } else {
            // At most eight hex digits remain, so the value fits in a u32.
            significant
                .chars()
                .filter_map(|digit| digit.to_digit(HEX_RADIX))
                .fold(0, |value, digit| (value << HEX_DIGIT_BITS) | digit)
        };
        Ok(TitleId(value))
    }
}

/// `NumberStyles.HexNumber` allows white space (U+0009 to U+000D, U+0020) at both ends and
/// trailing NUL characters; `str::trim` would also strip other Unicode white space.
fn trim_like_dotnet(text: &str) -> &str {
    let is_white_space = |c: char| matches!(c, '\u{9}'..='\u{D}' | ' ');
    text.trim_start_matches(is_white_space)
        .trim_end_matches(|c: char| is_white_space(c) || c == '\0')
}

impl fmt::Display for TitleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Result<String, QueryError> {
        TitleId::parse(text).map(|id| id.to_string())
    }

    #[test]
    fn title_id_parse_rules() {
        assert_eq!(parsed("4d5307e6"), Ok("4D5307E6".to_string()));
        assert_eq!(parsed("  1f "), Ok("0000001F".to_string()));
        assert_eq!(parsed(""), Err(QueryError::Empty));
        assert_eq!(parsed("   "), Err(QueryError::Empty));
        assert_eq!(parsed("123456789"), Err(QueryError::TooLong { len: 9 }));
        assert_eq!(parsed("0000000012"), Ok("00000012".to_string()));
        assert_eq!(parsed("  000000001F "), Ok("0000001F".to_string()));
        assert_eq!(parsed("100000000"), Err(QueryError::TooLong { len: 9 }));
        assert_eq!(
            parsed("xyz"),
            Err(QueryError::NotHex {
                text: "xyz".to_string()
            })
        );
        assert_eq!(
            parsed("0x12"),
            Err(QueryError::NotHex {
                text: "0x12".to_string()
            })
        );
    }

    #[test]
    fn trims_like_dotnet() {
        assert_eq!(parsed("\t\n1F\r "), Ok("0000001F".to_string()));
        assert_eq!(parsed("1F\0"), Ok("0000001F".to_string()));
        assert!(matches!(parsed("\u{A0}1F"), Err(QueryError::NotHex { .. })));
    }

    #[test]
    fn zero_parses() {
        assert_eq!(parsed("0"), Ok("00000000".to_string()));
    }
}
