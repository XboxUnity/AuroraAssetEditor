use std::fmt;

use serde::Deserialize;

use crate::cover_result::CoverResult;
use crate::query::CoverQuery;
use crate::url_encode::url_encode;

const NO_RATING: &str = "N/A";

/// The old app's `GetUnityUrl`: the TitleID as `X08`, or the keywords encoded untrimmed.
pub(crate) fn xboxunity_search_url(base: &str, query: &CoverQuery) -> String {
    match query {
        CoverQuery::TitleId(title_id) => format!("{base}{title_id}"),
        CoverQuery::Keywords(keywords) => format!("{base}{}", url_encode(keywords)),
    }
}

pub(crate) fn parse_xboxunity_covers(json: &[u8]) -> Result<Vec<CoverResult>, serde_json::Error> {
    let covers: Vec<XboxUnityCover> = serde_json::from_slice(json)?;
    Ok(covers.into_iter().map(CoverResult::from).collect())
}

/// The service sends the rating as a string; a number is accepted too.
#[derive(Deserialize)]
#[serde(untagged)]
enum Rating {
    Text(String),
    Number(serde_json::Number),
}

impl fmt::Display for Rating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.write_str(text),
            Self::Number(number) => write!(f, "{number}"),
        }
    }
}

#[derive(Deserialize)]
struct XboxUnityCover {
    name: Option<String>,
    #[serde(default)]
    official: bool,
    rating: Option<Rating>,
    url: String,
    thumbnail: Option<String>,
}

impl From<XboxUnityCover> for CoverResult {
    fn from(cover: XboxUnityCover) -> Self {
        let kind = if cover.official {
            "Official cover"
        } else {
            "Cover"
        };
        let name = cover.name.unwrap_or_default();
        let rating = cover
            .rating
            .map_or_else(|| NO_RATING.to_string(), |rating| rating.to_string());
        CoverResult {
            label: format!("{kind} for {name} Rating: {rating}"),
            image_url: cover.url,
            thumbnail_url: cover.thumbnail,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::query::TitleId;

    fn recorded(file_name: &str) -> Vec<u8> {
        let path = format!(
            "{}/../../fixtures/online/recorded/{file_name}",
            env!("CARGO_MANIFEST_DIR")
        );
        fs::read(&path).unwrap_or_else(|error| panic!("reading {path}: {error}"))
    }

    #[test]
    fn xboxunity_urls() {
        let base = "http://xboxunity.net/api/Covers/";
        let by_title = CoverQuery::TitleId(TitleId::new(0x4D5307E6));
        assert_eq!(
            xboxunity_search_url(base, &by_title),
            "http://xboxunity.net/api/Covers/4D5307E6"
        );
        let by_keywords = CoverQuery::keywords("halo 3").expect("non-blank keywords");
        assert_eq!(
            xboxunity_search_url(base, &by_keywords),
            "http://xboxunity.net/api/Covers/halo+3"
        );
    }

    #[test]
    fn parses_recorded_xboxunity_covers() {
        let json = recorded("xboxunity_title_4D5307E6.json");
        let recorded_count = serde_json::from_slice::<serde_json::Value>(&json)
            .expect("the recording is JSON")
            .as_array()
            .expect("the recording is an array")
            .len();

        let covers = parse_xboxunity_covers(&json).expect("the recording parses");

        assert_eq!(covers.len(), recorded_count);
        assert_eq!(
            covers[0],
            CoverResult {
                label: "Official cover for Halo 3 Rating: 5".to_string(),
                image_url: "http://assets.xboxunity.net/api/boxart/225".to_string(),
                thumbnail_url: Some("http://assets.xboxunity.net/api/boxartsm/225".to_string()),
            }
        );
        assert_eq!(covers[1].label, "Cover for Halo 3 Rating: 4");
    }

    #[test]
    fn parses_recorded_keyword_search_and_unknown_title() {
        let keyword_json = recorded("xboxunity_keywords_halo_3.json");
        let covers = parse_xboxunity_covers(&keyword_json).expect("the recording parses");
        assert!(!covers.is_empty());

        let unknown = parse_xboxunity_covers(&recorded("xboxunity_title_FFFFFFFF.json"))
            .expect("the recording parses");
        assert!(unknown.is_empty());
    }

    #[test]
    fn parses_null_and_numeric_fields() {
        let json = br#"[{"name":null,"official":false,"rating":null,"url":"u1","thumbnail":null},{"name":"Num","official":true,"rating":4,"url":"u2"}]"#;

        let covers = parse_xboxunity_covers(json).expect("both entries parse");

        assert_eq!(covers[0].label, "Cover for  Rating: N/A");
        assert_eq!(covers[0].thumbnail_url, None);
        assert_eq!(covers[1].label, "Official cover for Num Rating: 4");
        assert_eq!(covers[1].image_url, "u2");
    }

    #[test]
    fn invalid_xboxunity_json_is_error() {
        assert!(parse_xboxunity_covers(b"<html>").is_err());
        assert!(parse_xboxunity_covers(br#"[{"name":"x"}]"#).is_err());
    }
}
