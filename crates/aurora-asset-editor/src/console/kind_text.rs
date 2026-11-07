use aurora_asset::AssetKind;

/// How messages name a kind.
pub fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Cover => "cover",
        AssetKind::Background => "background",
        AssetKind::IconBanner => "icon & banner",
        AssetKind::Screenshots => "screenshots",
    }
}

/// "cover", "cover and background", "cover, background and screenshots".
pub fn kind_list(kinds: &[AssetKind]) -> String {
    let names: Vec<&str> = kinds.iter().map(|kind| kind_name(*kind)).collect();
    match names.split_last() {
        None => String::new(),
        Some((only, [])) => (*only).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// `text` with its first letter in capitals, for a sentence that starts with a kind list.
pub fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
