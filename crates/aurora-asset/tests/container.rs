mod support;

use aurora_asset::{
    AssetError, AssetFile, AssetKind, AssetSlot, ENTRY_COUNT, EncodedTexture, SCREENSHOT_COUNT,
    ScreenshotNumber, TextureHeader,
};

const DATA_START: usize = 2048;
const ENTRY_TABLE_START: usize = 20;
const ENTRY_LEN: usize = 64;

fn entry_field(bytes: &[u8], entry: usize, field_offset: usize) -> u32 {
    let at = ENTRY_TABLE_START + entry * ENTRY_LEN + field_offset;
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn be_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn texture(seed: u8, len: usize) -> EncodedTexture {
    let mut header = [seed; 52];
    header[0] = 0;
    EncodedTexture {
        header: TextureHeader::from_bytes(header),
        data: vec![seed; len],
    }
}

fn shot(number: u8) -> AssetSlot {
    AssetSlot::screenshot(number).unwrap()
}

fn number(number: u8) -> ScreenshotNumber {
    ScreenshotNumber::new(number).unwrap()
}

fn ph_cover() -> EncodedTexture {
    let fixture = support::manifest()
        .fixture
        .into_iter()
        .find(|fixture| fixture.name == "ph_cover")
        .unwrap();
    EncodedTexture {
        header: TextureHeader::from_bytes(support::header_bytes(&fixture.dxt_header)),
        data: support::dxt_video("ph_cover"),
    }
}

#[test]
fn new_file_serializes_to_header_only() {
    let bytes = AssetFile::new().to_bytes();
    assert_eq!(bytes.len(), DATA_START);
    assert_eq!(
        bytes[..12],
        [0x52, 0x58, 0x45, 0x41, 0, 0, 0, 1, 0, 0, 0, 0]
    );
    assert_eq!(be_u32(&bytes, 12), 0, "flags");
    assert_eq!(be_u32(&bytes, 16), 0, "screenshot count");
}

#[test]
fn round_trips_dll_encoded_cover() {
    let cover = ph_cover();
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(cover.clone()));

    let bytes = file.to_bytes();
    let cover_entry = AssetSlot::Cover.index();
    assert_eq!(entry_field(&bytes, cover_entry, 0), 0, "offset");
    assert_eq!(entry_field(&bytes, cover_entry, 4), cover.data.len() as u32);
    assert_eq!(entry_field(&bytes, cover_entry, 8), 0, "extended info");
    let header_start = ENTRY_TABLE_START + cover_entry * ENTRY_LEN + 12;
    assert_eq!(
        bytes[header_start..header_start + 52],
        cover.header.to_bytes()
    );
    assert_eq!(bytes[DATA_START..], cover.data[..]);
    assert_eq!(be_u32(&bytes, 8), cover.data.len() as u32, "data size");
    assert_eq!(be_u32(&bytes, 12), 1 << 2, "flags");

    assert_eq!(AssetFile::parse(&bytes).unwrap(), file);
}

#[test]
fn writes_like_old_app() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Icon, Some(texture(1, 10)));
    file.set(AssetSlot::Banner, Some(texture(2, 20)));
    file.set(shot(3), Some(texture(3, 30)));
    let mut bytes = file.to_bytes();

    let icon_entry = ENTRY_TABLE_START;
    let banner_entry = ENTRY_TABLE_START + ENTRY_LEN;
    bytes[icon_entry..icon_entry + 4].copy_from_slice(&777u32.to_be_bytes());
    bytes[banner_entry..banner_entry + 4].copy_from_slice(&999u32.to_be_bytes());
    bytes[icon_entry + 8..icon_entry + 12].copy_from_slice(&0xdead_beefu32.to_be_bytes());
    bytes[8..12].copy_from_slice(&12345u32.to_be_bytes());
    bytes[12..16].copy_from_slice(&0xffff_ffffu32.to_be_bytes());
    bytes[16..20].copy_from_slice(&99u32.to_be_bytes());
    bytes[1700..1800].fill(0xaa);

    let normalized = AssetFile::parse(&bytes).unwrap().to_bytes();
    assert_eq!(normalized, file.to_bytes());
    assert_eq!(entry_field(&normalized, 0, 0), 0);
    assert_eq!(entry_field(&normalized, 1, 0), 10);
    assert_eq!(entry_field(&normalized, shot(3).index(), 0), 30);
    assert_eq!(be_u32(&normalized, 8), 60);
    assert_eq!(be_u32(&normalized, 12), 0b11 | 1 << shot(3).index() as u32);
    assert_eq!(be_u32(&normalized, 16), 1);
}

#[test]
fn primary_kind_follows_priority() {
    let mut file = AssetFile::new();
    assert_eq!(file.primary_kind(), None);

    file.set(AssetSlot::Banner, Some(texture(1, 4)));
    assert_eq!(file.primary_kind(), Some(AssetKind::IconBanner));

    file.set(shot(2), Some(texture(2, 4)));
    assert_eq!(file.primary_kind(), Some(AssetKind::Screenshots));

    file.set(AssetSlot::Background, Some(texture(3, 4)));
    assert_eq!(file.primary_kind(), Some(AssetKind::Background));

    file.set(AssetSlot::Cover, Some(texture(4, 4)));
    assert_eq!(file.primary_kind(), Some(AssetKind::Cover));
}

#[test]
fn only_keeps_the_page_entries() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Icon, Some(texture(1, 4)));
    file.set(AssetSlot::Banner, Some(texture(2, 4)));
    file.set(AssetSlot::Cover, Some(texture(3, 4)));
    file.set(AssetSlot::Slot, Some(texture(4, 4)));
    file.set(AssetSlot::Background, Some(texture(5, 4)));
    file.set(shot(7), Some(texture(6, 4)));

    let icon_banner = file.only(AssetKind::IconBanner);
    assert!(icon_banner.get(AssetSlot::Icon).is_some());
    assert!(icon_banner.get(AssetSlot::Banner).is_some());
    assert!(icon_banner.get(AssetSlot::Cover).is_none());
    assert!(icon_banner.get(AssetSlot::Slot).is_none());
    assert!(icon_banner.get(shot(7)).is_none());

    let screenshots = file.only(AssetKind::Screenshots);
    assert!(screenshots.get(shot(7)).is_some());
    assert!(screenshots.get(AssetSlot::Icon).is_none());

    assert!(file.only(AssetKind::Cover).get(AssetSlot::Cover).is_some());
    assert!(
        file.only(AssetKind::Background)
            .get(AssetSlot::Background)
            .is_some()
    );
    assert!(
        file.only(AssetKind::Cover)
            .get(AssetSlot::Background)
            .is_none()
    );
}

#[test]
fn compact_screenshots_moves_bytes() {
    let mut file = AssetFile::new();
    let (a, b, c) = (texture(1, 8), texture(2, 16), texture(3, 24));
    file.set(shot(2), Some(a.clone()));
    file.set(shot(5), Some(b.clone()));
    file.set(shot(9), Some(c.clone()));

    file.compact_screenshots();

    assert_eq!(file.get(shot(1)), Some(&a));
    assert_eq!(file.get(shot(2)), Some(&b));
    assert_eq!(file.get(shot(3)), Some(&c));
    for empty in 4..=20 {
        assert!(file.get(shot(empty)).is_none(), "slot {empty}");
    }
}

#[test]
fn move_screenshot_reorders_bytes() {
    let mut file = AssetFile::new();
    let (a, b, c) = (texture(1, 8), texture(2, 16), texture(3, 24));
    file.set(shot(1), Some(a.clone()));
    file.set(shot(2), Some(b.clone()));
    file.set(shot(3), Some(c.clone()));

    file.move_screenshot(number(3), number(1));

    assert_eq!(file.get(shot(1)), Some(&c));
    assert_eq!(file.get(shot(2)), Some(&a));
    assert_eq!(file.get(shot(3)), Some(&b));

    file.move_screenshot(number(1), number(3));
    assert_eq!(file.get(shot(1)), Some(&a));
    assert_eq!(file.get(shot(2)), Some(&b));
    assert_eq!(file.get(shot(3)), Some(&c));
}

#[test]
fn move_screenshot_from_empty_slot_changes_nothing() {
    let mut file = AssetFile::new();
    file.set(shot(1), Some(texture(1, 8)));
    file.set(shot(3), Some(texture(3, 8)));
    let before = file.clone();

    file.move_screenshot(number(2), number(1));
    file.move_screenshot(number(2), number(3));

    assert_eq!(file, before);
}

#[test]
fn screenshot_numbers_accept_only_one_to_twenty() {
    assert!(ScreenshotNumber::new(0).is_none());
    assert!(ScreenshotNumber::new(21).is_none());
    assert!(AssetSlot::screenshot(0).is_none());
    assert_eq!(AssetSlot::screenshot(1).unwrap().index(), 5);
    assert_eq!(AssetSlot::screenshot(20).unwrap().index(), 24);
}

#[test]
fn slot_indices_round_trip() {
    for index in 0..25 {
        assert_eq!(AssetSlot::from_index(index).unwrap().index(), index);
    }
    assert!(AssetSlot::from_index(25).is_none());
}

#[test]
fn keeps_unsupported_entry_bytes() {
    let crafted = support::manifest()
        .crafted
        .into_iter()
        .find(|case| case.name.contains("tiled"))
        .expect("a crafted tiled case");
    let tiled = EncodedTexture {
        header: TextureHeader::from_bytes(support::header_bytes(&crafted.header)),
        data: support::crafted_video(&crafted),
    };
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(tiled));
    let bytes = file.to_bytes();

    let mut reparsed = AssetFile::parse(&bytes).unwrap();

    assert_eq!(reparsed, file);
    assert_eq!(reparsed.to_bytes(), bytes);
}

#[test]
fn rejects_short_file() {
    assert_eq!(
        AssetFile::parse(&[0; 2047]),
        Err(AssetError::TooShort { len: 2047 })
    );
}

#[test]
fn rejects_bad_magic_and_version() {
    let mut bytes = AssetFile::new().to_bytes();
    bytes[0] = 0;
    assert_eq!(
        AssetFile::parse(&bytes),
        Err(AssetError::BadMagic(0x0058_4541))
    );

    let mut bytes = AssetFile::new().to_bytes();
    bytes[7] = 2;
    assert_eq!(
        AssetFile::parse(&bytes),
        Err(AssetError::UnsupportedVersion(2))
    );
}

#[test]
fn rejects_entry_running_past_end() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture(1, 16)));
    let mut bytes = file.to_bytes();
    let size_at = ENTRY_TABLE_START + AssetSlot::Cover.index() * ENTRY_LEN + 4;
    bytes[size_at..size_at + 4].copy_from_slice(&u32::MAX.to_be_bytes());

    let error = AssetFile::parse(&bytes).unwrap_err();

    assert!(
        matches!(
            error,
            AssetError::EntryOutOfBounds { slot: AssetSlot::Cover, len, .. } if len == bytes.len()
        ),
        "{error:?}"
    );
}

#[test]
fn rejects_second_entry_past_end_after_first_consumed_data() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Icon, Some(texture(1, 16)));
    file.set(AssetSlot::Banner, Some(texture(2, 16)));
    let mut bytes = file.to_bytes();
    bytes.truncate(bytes.len() - 1);

    assert!(matches!(
        AssetFile::parse(&bytes),
        Err(AssetError::EntryOutOfBounds {
            slot: AssetSlot::Banner,
            ..
        })
    ));
}

fn patch_entry(bytes: &mut [u8], entry: usize, offset: u32, header_fill: u8) {
    let start = ENTRY_TABLE_START + entry * ENTRY_LEN;
    bytes[start..start + 4].copy_from_slice(&offset.to_be_bytes());
    bytes[start + 12..start + 64].fill(header_fill);
}

fn entry_bytes(bytes: &[u8], entry: usize) -> &[u8] {
    let start = ENTRY_TABLE_START + entry * ENTRY_LEN;
    &bytes[start..start + ENTRY_LEN]
}

#[test]
fn empty_entry_residue_survives_load_and_save() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture(1, 12)));
    let mut bytes = file.to_bytes();
    patch_entry(&mut bytes, AssetSlot::Banner.index(), 0x1234, 0x7e);
    let mut loaded = AssetFile::parse(&bytes).unwrap();

    assert_eq!(loaded.to_bytes(), bytes);
}

#[test]
fn removed_screenshot_keeps_stale_offset_with_zero_header() {
    let mut file = AssetFile::new();
    file.set(shot(1), Some(texture(1, 8)));
    file.set(shot(2), Some(texture(2, 16)));
    file.set(shot(3), Some(texture(3, 24)));
    let mut loaded = AssetFile::parse(&file.to_bytes()).unwrap();

    loaded.set(shot(2), None);
    let bytes = loaded.to_bytes();

    let removed = entry_bytes(&bytes, shot(2).index());
    assert_eq!(be_u32(removed, 0), 8, "stale offset");
    assert_eq!(be_u32(removed, 4), 0, "size");
    assert!(removed[12..].iter().all(|byte| *byte == 0), "header");
    assert_eq!(entry_field(&bytes, shot(3).index(), 0), 8, "recomputed");
}

#[test]
fn only_starts_fresh_and_copies_owned_headers() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Icon, Some(texture(1, 8)));
    file.set(AssetSlot::Cover, Some(texture(2, 8)));
    let mut bytes = file.to_bytes();
    patch_entry(&mut bytes, AssetSlot::Banner.index(), 0x55, 0x7e);
    patch_entry(&mut bytes, AssetSlot::Background.index(), 0x66, 0x7f);
    let loaded = AssetFile::parse(&bytes).unwrap();

    let mut page = loaded.only(AssetKind::IconBanner);
    let page_bytes = page.to_bytes();

    let banner = entry_bytes(&page_bytes, AssetSlot::Banner.index());
    assert_eq!(be_u32(banner, 0), 0, "offset starts fresh");
    assert!(
        banner[12..].iter().all(|byte| *byte == 0x7e),
        "owned header"
    );
    let background = entry_bytes(&page_bytes, AssetSlot::Background.index());
    assert!(background.iter().all(|byte| *byte == 0), "unowned is zero");
    assert!(page.get(AssetSlot::Icon).is_some());
}

#[test]
fn truncated_files_never_panic() {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Icon, Some(texture(1, 16)));
    file.set(shot(4), Some(texture(2, 24)));
    let bytes = file.to_bytes();

    for len in 0..bytes.len() {
        assert!(AssetFile::parse(&bytes[..len]).is_err(), "length {len}");
    }
    assert!(AssetFile::parse(&bytes).is_ok());
}

#[test]
fn compact_after_removal_matches_old_app_bytes() {
    let mut file = AssetFile::new();
    for number in 1..=3 {
        file.set(shot(number), Some(texture(number, 8 * usize::from(number))));
    }
    let mut bytes = file.to_bytes();
    patch_entry(&mut bytes, shot(4).index(), 0x44, 0x7e);
    patch_entry(&mut bytes, shot(5).index(), 0x55, 0x7f);
    let mut loaded = AssetFile::parse(&bytes).unwrap();

    loaded.set(shot(2), None);
    loaded.compact_screenshots();
    let saved = loaded.to_bytes();

    for (number, offset) in [(1, 0), (2, 8)] {
        assert_eq!(entry_field(&saved, shot(number).index(), 0), offset);
    }
    let emptied = entry_bytes(&saved, shot(3).index());
    assert_eq!(be_u32(emptied, 0), 24, "slot 3 keeps its own stored offset");
    assert!(emptied[12..].iter().all(|byte| *byte == 0));
    for (number, offset) in [(4, 0x44), (5, 0x55)] {
        let slot = entry_bytes(&saved, shot(number).index());
        assert_eq!(be_u32(slot, 0), offset, "slot {number}");
        assert!(slot[12..].iter().all(|byte| *byte == 0), "slot {number}");
    }
}

#[test]
fn compact_keeps_each_slots_offset_across_saves() {
    let mut file = AssetFile::new();
    for number in 1..=3 {
        file.set(shot(number), Some(texture(number, 8 * usize::from(number))));
    }
    file.to_bytes();
    file.set(shot(2), None);
    file.to_bytes();
    file.compact_screenshots();
    let saved = file.to_bytes();

    assert_eq!(entry_field(&saved, shot(3).index(), 0), 8);
    assert_eq!(entry_field(&saved, shot(3).index(), 4), 0);
}

#[test]
fn move_screenshot_zeroes_header_of_emptied_slot() {
    let mut file = AssetFile::new();
    file.set(shot(1), Some(texture(1, 8)));
    file.set(shot(2), Some(texture(2, 8)));
    let mut bytes = file.to_bytes();
    patch_entry(&mut bytes, shot(3).index(), 0x33, 0x7e);
    let mut loaded = AssetFile::parse(&bytes).unwrap();

    loaded.move_screenshot(number(1), number(3));
    let saved = loaded.to_bytes();

    let slot3 = entry_bytes(&saved, shot(3).index());
    assert_eq!(be_u32(slot3, 4), 8, "texture moved to slot 3");
    assert_eq!(loaded.get(shot(1)).unwrap().data, vec![2; 8]);

    assert!(loaded.get(shot(2)).is_none());
    let emptied = entry_bytes(&saved, shot(2).index());
    assert_eq!(be_u32(emptied, 0), 8, "slot 2 keeps its own stored offset");
    assert_eq!(be_u32(emptied, 4), 0, "slot 2 stores no data");
    assert!(emptied[12..].iter().all(|byte| *byte == 0), "slot 2 header");
}

#[test]
fn file_prefixes_match_aurora_names() {
    // Aurora: Tools/Content/ContentItem.cpp, "%sGC%08X.asset" etc.
    let prefixes: Vec<_> = AssetKind::ALL
        .iter()
        .map(|kind| kind.file_prefix())
        .collect();
    assert_eq!(prefixes, ["GC", "BK", "GL", "SS"]);
    assert_eq!(
        AssetKind::ALL,
        [
            AssetKind::Cover,
            AssetKind::Background,
            AssetKind::IconBanner,
            AssetKind::Screenshots
        ]
    );
}

#[test]
fn replace_kind_keeps_offsets_like_old_set_image() {
    let mut target = AssetFile::new();
    target.set(AssetSlot::Cover, Some(texture(9, 10)));
    target.set(shot(1), Some(texture(1, 8)));
    target.to_bytes();
    target.set(shot(1), None);

    let mut source_bytes_file = AssetFile::new();
    source_bytes_file.set(shot(2), Some(texture(2, 16)));
    let mut source_bytes = source_bytes_file.to_bytes();
    patch_entry(&mut source_bytes, shot(3).index(), 0x77, 0x6d);
    let source = AssetFile::parse(&source_bytes).unwrap();

    target.replace_kind(AssetKind::Screenshots, &source);
    let bytes = target.to_bytes();

    let first = entry_bytes(&bytes, shot(1).index());
    assert_eq!(
        be_u32(first, 0),
        10,
        "slot 1 keeps the offset recorded by the first save"
    );
    assert!(
        first[12..].iter().all(|byte| *byte == 0),
        "slot 1 takes the source's zero header"
    );

    let third = entry_bytes(&bytes, shot(3).index());
    assert_eq!(be_u32(third, 4), 0, "slot 3 stays empty");
    assert!(
        third[12..].iter().all(|byte| *byte == 0x6d),
        "slot 3 takes the source's residue header"
    );

    assert_eq!(
        entry_field(&bytes, shot(2).index(), 0),
        10,
        "slot 2 is recomputed after the cover"
    );
    assert_eq!(entry_field(&bytes, AssetSlot::Cover.index(), 0), 0);
    assert_eq!(target.get(AssetSlot::Cover), Some(&texture(9, 10)));
    assert_eq!(target.get(shot(2)), Some(&texture(2, 16)));
}

#[test]
fn replace_kind_copies_only_owned_slots() {
    let mut source = AssetFile::new();
    for (seed, slot) in [
        (1, AssetSlot::Icon),
        (2, AssetSlot::Banner),
        (3, AssetSlot::Cover),
        (4, AssetSlot::Slot),
        (5, AssetSlot::Background),
        (6, shot(1)),
        (7, shot(20)),
    ] {
        source.set(slot, Some(texture(seed, 4)));
    }

    for kind in AssetKind::ALL {
        let mut target = AssetFile::new();
        target.replace_kind(kind, &source);
        for index in 0..25 {
            let slot = AssetSlot::from_index(index).unwrap();
            let expected = kind.slots().any(|owned| owned == slot) && source.get(slot).is_some();
            assert_eq!(
                target.get(slot).is_some(),
                expected,
                "{kind:?} slot {index}"
            );
        }
    }

    let mut cleared = source.clone();
    cleared.replace_kind(AssetKind::Cover, &AssetFile::new());
    assert!(cleared.get(AssetSlot::Cover).is_none());
    assert!(cleared.get(AssetSlot::Icon).is_some());
}

#[test]
fn kind_slots_are_disjoint_and_match_entries() {
    let mut owners = [0usize; 25];
    for kind in AssetKind::ALL {
        for slot in kind.slots() {
            owners[slot.index()] += 1;
        }
    }
    for (index, count) in owners.iter().enumerate() {
        let expected = usize::from(index != AssetSlot::Slot.index());
        assert_eq!(*count, expected, "entry {index}");
    }
    assert_eq!(AssetKind::Screenshots.slots().count(), SCREENSHOT_COUNT);
    assert_eq!(ENTRY_COUNT, 25);
    assert_eq!(ScreenshotNumber::ALL.len(), SCREENSHOT_COUNT);
    assert_eq!(ScreenshotNumber::ALL[0].get(), 1);
    assert_eq!(
        ScreenshotNumber::ALL[SCREENSHOT_COUNT - 1].get() as usize,
        SCREENSHOT_COUNT
    );

    for checked in AssetKind::ALL {
        for held in AssetKind::ALL {
            for slot in held.slots() {
                let mut file = AssetFile::new();
                file.set(slot, Some(texture(1, 4)));
                assert_eq!(
                    file.has_entries_for(checked),
                    checked == held,
                    "{checked:?} vs {slot:?}"
                );
            }
        }
    }

    let mut unused_slot_only = AssetFile::new();
    unused_slot_only.set(AssetSlot::Slot, Some(texture(1, 4)));
    assert!(
        AssetKind::ALL
            .iter()
            .all(|kind| !unused_slot_only.has_entries_for(*kind))
    );
}
