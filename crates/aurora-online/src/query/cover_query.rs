use super::{QueryError, TitleId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverQuery {
    TitleId(TitleId),
    Keywords(String),
}

impl CoverQuery {
    /// The text is kept as typed (the old app encoded it untrimmed); only blank text is refused.
    pub fn keywords(text: &str) -> Result<CoverQuery, QueryError> {
        if text.trim().is_empty() {
            return Err(QueryError::Empty);
        }
        Ok(CoverQuery::Keywords(text.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_must_not_be_empty() {
        assert_eq!(CoverQuery::keywords(""), Err(QueryError::Empty));
        assert_eq!(CoverQuery::keywords("  "), Err(QueryError::Empty));
        assert_eq!(
            CoverQuery::keywords(" halo 3 "),
            Ok(CoverQuery::Keywords(" halo 3 ".to_string()))
        );
    }
}
