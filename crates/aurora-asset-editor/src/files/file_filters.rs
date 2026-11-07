use aurora_image::IMPORT_EXTENSIONS;

use crate::platform::FileFilter;

/// The old `FilterIndex = 5`, counted from 0 here: "Aurora Asset Files (*.asset)".
pub const ASSET_OPEN_DEFAULT_FILTER: usize = 4;

/// "All supported images" is the first entry.
pub const IMAGE_OPEN_DEFAULT_FILTER: usize = 0;

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

/// Every image format the editor reads, then one entry per format, then every file.
pub fn image_open_filters() -> Vec<FileFilter> {
    let mut filters = vec![extension_filter("All supported images", &IMPORT_EXTENSIONS)];
    filters.extend(
        [
            ("PNG", &["png"][..]),
            ("JPEG", &["jpg", "jpeg"]),
            ("BMP", &["bmp"]),
            ("GIF", &["gif"]),
            ("TIFF", &["tif", "tiff"]),
            ("WebP", &["webp"]),
            ("TGA", &["tga"]),
            ("ICO", &["ico"]),
            ("QOI", &["qoi"]),
            ("DDS", &["dds"]),
        ]
        .map(|(name, extensions)| extension_filter(name, extensions)),
    );
    filters.push(FileFilter {
        name: "All files".to_owned(),
        patterns: vec!["*".to_owned()],
    });
    filters
}

fn extension_filter(name: &str, extensions: &[&str]) -> FileFilter {
    FileFilter {
        name: name.to_owned(),
        patterns: extensions
            .iter()
            .map(|extension| format!("*.{extension}"))
            .collect(),
    }
}
