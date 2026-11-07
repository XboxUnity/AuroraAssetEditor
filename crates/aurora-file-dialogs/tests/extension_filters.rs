use aurora_file_dialogs::{DialogFilter, extension_filters};

fn filter(name: &str, patterns: &[&str]) -> DialogFilter {
    DialogFilter {
        name: name.to_owned(),
        patterns: patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect(),
    }
}

fn entry(name: &str, extensions: &[&str]) -> (String, Vec<String>) {
    (
        name.to_owned(),
        extensions
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
    )
}

#[test]
fn extension_filters_reduce_patterns_to_extensions() {
    // The names of `asset_open_filters` in aurora-asset-editor's files/file_filters.rs.
    let old_open_filters = [
        filter("Game Cover/Boxart Asset File (GC*.asset)", &["GC*.asset"]),
        filter("Background Asset File (BK*.asset)", &["BK*.asset"]),
        filter("Icon/Banner Asset File (GL*.asset)", &["GL*.asset"]),
        filter("Screenshot Asset File (SS*.asset)", &["SS*.asset"]),
        filter("Aurora Asset Files (*.asset)", &["*.asset"]),
        filter("FSD Assets Files (*.assets)", &["*.assets"]),
        filter("All files", &["*"]),
    ];

    assert_eq!(
        extension_filters(&old_open_filters, 4),
        vec![
            entry("Aurora Asset Files (*.asset)", &["asset"]),
            entry("FSD Assets Files (*.assets)", &["assets"]),
        ]
    );

    let images = [
        filter("PNG", &["*.png"]),
        filter("JPEG", &["*.jpg", "*.jpeg"]),
    ];
    assert_eq!(
        extension_filters(&images, 1),
        vec![entry("JPEG", &["jpg", "jpeg"]), entry("PNG", &["png"])]
    );
}
