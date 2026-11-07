use std::sync::LazyLock;

use regex::Regex;

use crate::cover_result::CoverResult;
use crate::query::TitleId;

/// The old app's pattern: a directory link whose target and text agree, which leaves out `../`.
const DIRECTORY_LINK_PATTERN: &str = r#"<a href="(.+)/">(.+)/</a>"#;

static DIRECTORY_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(DIRECTORY_LINK_PATTERN).expect("the directory link pattern is a valid regex")
});

pub(crate) fn archive_listing_url(base: &str, title_id: TitleId) -> String {
    format!("{base}{title_id}/")
}

pub(crate) fn parse_archive_variants(html: &str) -> Vec<String> {
    DIRECTORY_LINK
        .captures_iter(html)
        .filter(|link| link[1] == link[2])
        .map(|link| link[1].to_string())
        .collect()
}

pub(crate) fn archive_cover(base: &str, title_id: TitleId, variant: &str) -> CoverResult {
    CoverResult {
        label: format!("TitleID: {title_id} - Variant: {variant}"),
        image_url: format!("{base}{title_id}/{variant}/boxart.png"),
        thumbnail_url: None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const BASE: &str =
        "https://archive.org/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/";

    fn halo_3() -> TitleId {
        TitleId::new(0x4D5307E6)
    }

    #[test]
    fn parses_recorded_archive_variants() {
        let path = format!(
            "{}/../../fixtures/online/recorded/archive_listing_4D5307E6.html",
            env!("CARGO_MANIFEST_DIR")
        );
        let html = fs::read_to_string(&path).expect("the recording is UTF-8 text");

        let variants = parse_archive_variants(&html);

        assert_eq!(variants.len(), 28);
        assert_eq!(variants.first().map(String::as_str), Some("11234"));
        assert_eq!(variants.last().map(String::as_str), Some("9963"));
        assert!(variants.iter().any(|variant| variant == "225"));
        assert!(!variants.iter().any(|variant| variant.contains("..")));

        let cover = archive_cover(BASE, halo_3(), "225");
        assert_eq!(cover.label, "TitleID: 4D5307E6 - Variant: 225");
        assert_eq!(cover.image_url, format!("{BASE}4D5307E6/225/boxart.png"));
        assert_eq!(cover.thumbnail_url, None);
    }

    #[test]
    fn archive_variants_need_matching_text() {
        let html =
            "<a href=\"A/\">B/</a>\n<a href=\"boxart.png\">boxart.png</a>\n<a href=\"C/\">C/</a>\n";
        assert_eq!(parse_archive_variants(html), ["C"]);
    }

    #[test]
    fn archive_listing_url_uses_the_title_folder() {
        assert_eq!(
            archive_listing_url(BASE, halo_3()),
            format!("{BASE}4D5307E6/")
        );
    }
}
