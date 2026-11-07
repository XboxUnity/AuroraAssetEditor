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
    let old_open_filters = [
        filter("Game cover asset files (GC*.asset)", &["GC*.asset"]),
        filter("Background asset files (BK*.asset)", &["BK*.asset"]),
        filter("Icon & banner asset files (GL*.asset)", &["GL*.asset"]),
        filter("Screenshot asset files (SS*.asset)", &["SS*.asset"]),
        filter("Aurora asset files (*.asset)", &["*.asset"]),
        filter("FSD asset files (*.assets)", &["*.assets"]),
        filter("All files", &["*"]),
    ];

    assert_eq!(
        extension_filters(&old_open_filters, 4),
        vec![
            entry("Aurora asset files (*.asset)", &["asset"]),
            entry("FSD asset files (*.assets)", &["assets"]),
            entry("All files", &["*"]),
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
