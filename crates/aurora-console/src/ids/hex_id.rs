use std::fmt;

const HEX_ID_LEN: usize = 8;

/// Exactly eight hex digits, kept in the case they were given (Aurora's folder and file names use the dialog's text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    WrongLength { len: usize },
    NotHex { text: String },
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { len } => {
                write!(f, "an ID has {HEX_ID_LEN} hex digits, not {len}")
            }
            Self::NotHex { text } => write!(f, "\"{text}\" is not made of hex digits"),
        }
    }
}

impl std::error::Error for IdError {}

impl HexId {
    pub fn parse(text: &str) -> Result<HexId, IdError> {
        let len = text.chars().count();
        if len != HEX_ID_LEN {
            return Err(IdError::WrongLength { len });
        }
        if !text.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(IdError::NotHex {
                text: text.to_string(),
            });
        }
        Ok(HexId(text.to_string()))
    }

    pub fn from_database_value(value: i64) -> HexId {
        // Mirrors the old ((int)(long)x).ToString("X08").
        HexId(format!("{:08X}", value as i32 as u32))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HexId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_id_parse_rules() {
        assert_eq!(HexId::parse("4D5307E6").unwrap().as_str(), "4D5307E6");
        assert_eq!(HexId::parse("4d5307e6").unwrap().as_str(), "4d5307e6");
        assert_eq!(
            HexId::parse("4D5307E"),
            Err(IdError::WrongLength { len: 7 })
        );
        assert_eq!(
            HexId::parse("4D5307E6A"),
            Err(IdError::WrongLength { len: 9 })
        );
        assert_eq!(
            HexId::parse("4D5307G6"),
            Err(IdError::NotHex {
                text: "4D5307G6".to_string()
            })
        );
        assert_eq!(HexId::parse(""), Err(IdError::WrongLength { len: 0 }));
    }

    #[test]
    fn hex_id_from_database_value_truncates_like_int_cast() {
        let cases = [
            (0x4D53_07E6, "4D5307E6"),
            (18, "00000012"),
            (-1, "FFFFFFFF"),
            (0x1_4D53_07E6, "4D5307E6"),
            (i64::MIN, "00000000"),
        ];
        for (value, expected) in cases {
            assert_eq!(HexId::from_database_value(value).as_str(), expected);
        }
    }
}
