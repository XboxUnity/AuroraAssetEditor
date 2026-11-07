use asset_fixtures::{
    BACKGROUND, BANNER, BOXART, COVER, ENTRY_TABLE_OFFSET, FsdBuilder, HEADER_LEN, ICON, PREVIEW,
    SCREENSHOT, SLOT,
};
use aurora_asset::fsd::{FsdError, FsdFile, FsdImage};

fn encoded(data: &[u8]) -> Option<FsdImage> {
    Some(FsdImage::Encoded(data.to_vec()))
}

#[test]
fn parses_all_types() {
    let bytes = FsdBuilder::new()
        .entry(COVER, b"cover")
        .entry(BACKGROUND, b"background")
        .entry(ICON, b"icon")
        .entry(BANNER, b"banner")
        .screenshot(b"shot one")
        .screenshot(b"shot two")
        .build();
    let file = FsdFile::parse(&bytes).unwrap();
    assert_eq!(file.cover, encoded(b"cover"));
    assert_eq!(file.background, encoded(b"background"));
    assert_eq!(file.icon, encoded(b"icon"));
    assert_eq!(file.banner, encoded(b"banner"));
    assert_eq!(
        file.screenshots,
        vec![
            FsdImage::Encoded(b"shot one".to_vec()),
            FsdImage::Encoded(b"shot two".to_vec()),
        ]
    );
}

#[test]
fn first_non_empty_entry_of_a_type_wins() {
    let bytes = FsdBuilder::new()
        .entry(ICON, b"")
        .entry(ICON, b"first")
        .entry(ICON, b"second")
        .build();
    assert_eq!(FsdFile::parse(&bytes).unwrap().icon, encoded(b"first"));
}

#[test]
fn screenshots_fall_back_to_type_0x20_entries() {
    let bytes = FsdBuilder::new()
        .entry(SCREENSHOT, b"shot one")
        .entry(ICON, b"icon")
        .entry(SCREENSHOT, b"shot two")
        .build();
    let file = FsdFile::parse(&bytes).unwrap();
    assert_eq!(
        file.screenshots,
        vec![
            FsdImage::Encoded(b"shot one".to_vec()),
            FsdImage::Encoded(b"shot two".to_vec()),
        ]
    );
}

#[test]
fn screenshot_table_takes_precedence_over_type_0x20_entries() {
    let bytes = FsdBuilder::new()
        .entry(SCREENSHOT, b"entry shot")
        .screenshot(b"table shot")
        .build();
    let file = FsdFile::parse(&bytes).unwrap();
    assert_eq!(
        file.screenshots,
        vec![FsdImage::Encoded(b"table shot".to_vec())]
    );
}

#[test]
fn ignores_types_0x08_0x10_0x40() {
    let bytes = FsdBuilder::new()
        .entry(BOXART, b"boxart")
        .entry(PREVIEW, b"preview")
        .entry(SLOT, b"slot")
        .build();
    let file = FsdFile::parse(&bytes).unwrap();
    assert_eq!(file, FsdFile::default());
}

#[test]
fn classifies_dds_by_magic() {
    let bytes = FsdBuilder::new()
        .entry(COVER, b"DDS payload")
        .entry(ICON, b"DXS payload")
        .entry(BANNER, b"\x89PNG")
        .build();
    let file = FsdFile::parse(&bytes).unwrap();
    assert_eq!(file.cover, Some(FsdImage::Dds(b"DDS payload".to_vec())));
    assert_eq!(file.icon, encoded(b"DXS payload"));
    assert_eq!(file.banner, encoded(b"\x89PNG"));
}

#[test]
fn rejects_short_file() {
    let bytes = FsdBuilder::new().build();
    assert_eq!(
        FsdFile::parse(&bytes[..HEADER_LEN - 1]),
        Err(FsdError::TooShort {
            len: HEADER_LEN - 1
        })
    );
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = FsdBuilder::new().build();
    bytes[0] = b'X';
    assert_eq!(FsdFile::parse(&bytes), Err(FsdError::BadMagic(0x5853_4441)));
}

#[test]
fn rejects_unsupported_version() {
    let mut bytes = FsdBuilder::new().build();
    bytes[7] = 2;
    assert_eq!(FsdFile::parse(&bytes), Err(FsdError::UnsupportedVersion(2)));
}

#[test]
fn rejects_entry_out_of_bounds() {
    let mut bytes = FsdBuilder::new().entry(BOXART, b"boxart").build();
    bytes.pop();
    assert_eq!(
        FsdFile::parse(&bytes),
        Err(FsdError::EntryOutOfBounds { index: 0 })
    );

    let mut bytes = FsdBuilder::new()
        .screenshot(b"one")
        .screenshot(b"two")
        .build();
    bytes.pop();
    assert_eq!(
        FsdFile::parse(&bytes),
        Err(FsdError::EntryOutOfBounds { index: 11 })
    );
}

#[test]
fn rejects_entry_whose_end_overflows() {
    let mut bytes = FsdBuilder::new().entry(ICON, b"icon").build();
    let size_field = ENTRY_TABLE_OFFSET + 8;
    bytes[size_field..size_field + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        FsdFile::parse(&bytes),
        Err(FsdError::EntryOutOfBounds { index: 0 })
    );
}
