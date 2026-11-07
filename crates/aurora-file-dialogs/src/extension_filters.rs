use super::DialogFilter;

const ALL_FILES_PATTERN: &str = "*";
const EXTENSION_PATTERN_PREFIX: &str = "*.";

/// The filters as name and extensions, for dialogs that filter by extension only (macOS): `*.asset` gives
/// `asset` and `*` stays `*`. An entry with a pattern such as `GC*.asset` cannot be expressed and is left out.
/// The default filter comes first, because such dialogs have no way to choose a default.
pub fn extension_filters(
    filters: &[DialogFilter],
    default_filter: usize,
) -> Vec<(String, Vec<String>)> {
    let mut reduced: Vec<(bool, (String, Vec<String>))> = filters
        .iter()
        .enumerate()
        .filter_map(|(index, filter)| {
            extensions_of(&filter.patterns)
                .map(|extensions| (index == default_filter, (filter.name.clone(), extensions)))
        })
        .collect();
    reduced.sort_by_key(|(is_default, _)| !is_default);
    reduced.into_iter().map(|(_, entry)| entry).collect()
}

fn extensions_of(patterns: &[String]) -> Option<Vec<String>> {
    patterns
        .iter()
        .map(|pattern| extension_of(pattern))
        .collect()
}

fn extension_of(pattern: &str) -> Option<String> {
    if pattern == ALL_FILES_PATTERN {
        return Some(ALL_FILES_PATTERN.to_owned());
    }
    pattern
        .strip_prefix(EXTENSION_PATTERN_PREFIX)
        .filter(|extension| !extension.is_empty() && !extension.contains(['*', '?', '.']))
        .map(str::to_owned)
}
