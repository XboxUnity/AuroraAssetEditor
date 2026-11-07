use crate::platform::FileFilter;

/// The old `FilterIndex = 5`, counted from 0 here: "Aurora Asset Files (*.asset)".
pub const ASSET_OPEN_DEFAULT_FILTER: usize = 4;

/// The old app's seven filters in the old order. The old names carried a stray "(defaultFilename)", which is left out.
pub fn asset_open_filters() -> Vec<FileFilter> {
    [
        ("Game Cover/Boxart Asset File (GC*.asset)", "GC*.asset"),
        ("Background Asset File (BK*.asset)", "BK*.asset"),
        ("Icon/Banner Asset File (GL*.asset)", "GL*.asset"),
        ("Screenshot Asset File (SS*.asset)", "SS*.asset"),
        ("Aurora Asset Files (*.asset)", "*.asset"),
        ("FSD Assets Files (*.assets)", "*.assets"),
        ("All Files(*)", "*"),
    ]
    .into_iter()
    .map(|(name, pattern)| FileFilter {
        name: name.to_owned(),
        patterns: vec![pattern.to_owned()],
    })
    .collect()
}
