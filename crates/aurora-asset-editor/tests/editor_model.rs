use aurora_asset::{
    AssetFile, AssetKind, AssetSlot, Compression, DecodeError, EncodedTexture, RgbaImage,
    ScreenshotNumber, TextureHeader, decode, encode,
};
use aurora_asset_editor::editor::{
    AssetDocument, EditorPages, PreparedImage, SlotPreview, decode_kind, texture_caption,
};
use aurora_image::{ImageTarget, encoder_pixels, fit_to_target};
use editor_test_support::{asset_file_with, placeholder};
use image::Rgba;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
/// The tiled flag is the top bit of the first fetch dword (header dword 7).
const TILED_FLAG_BYTE: usize = 7 * 4;
const TILED_FLAG: u8 = 0x80;
/// The second fetch dword (header dword 8, big-endian) holds the data format in its low six bits
/// and the byte order in the two above them.
const FETCH_1_LOW_BYTE: usize = 8 * 4 + 3;
const ENDIAN_SHIFT: u32 = 6;
const DXT1_DATA_FORMAT: u8 = 0x12;
const DXT4_5_DATA_FORMAT: u8 = 0x14;
const ENDIAN_8IN16: u8 = 1;
const ENDIAN_8IN32: u8 = 2;

fn shot(number: u8) -> AssetSlot {
    AssetSlot::screenshot(number).unwrap()
}

fn number(number: u8) -> ScreenshotNumber {
    ScreenshotNumber::new(number).unwrap()
}

fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

fn prepared(width: u32, height: u32, color: [u8; 4]) -> PreparedImage {
    PreparedImage::without_resize(solid(width, height, color), Compression::Dxt).unwrap()
}

fn screenshot_document(colors: &[(u8, [u8; 4])]) -> AssetDocument {
    let mut document = AssetDocument::new(AssetKind::Screenshots);
    for (position, color) in colors {
        document.set_prepared(shot(*position), prepared(8, 8, *color));
    }
    document
}

fn pixels_of(document: &AssetDocument, slot: AssetSlot) -> RgbaImage {
    match document.preview(slot) {
        SlotPreview::Pixels(pixels) => pixels.clone(),
        other => panic!("expected pixels, got {other:?}"),
    }
}

fn stored_texture(document: &mut AssetDocument, slot: AssetSlot) -> Option<EncodedTexture> {
    AssetFile::parse(&document.to_bytes())
        .unwrap()
        .get(slot)
        .cloned()
}

fn tiled_texture() -> EncodedTexture {
    let mut texture = encode(&solid(8, 8, [1, 2, 3, 255]), Compression::Dxt).unwrap();
    let mut header = texture.header.to_bytes();
    header[TILED_FLAG_BYTE] |= TILED_FLAG;
    texture.header = TextureHeader::from_bytes(header);
    texture
}

fn dxt_header_with(data_format: u8, endian: u8) -> TextureHeader {
    let mut header = encode(&solid(8, 8, [1, 2, 3, 255]), Compression::Dxt)
        .unwrap()
        .header
        .to_bytes();
    header[FETCH_1_LOW_BYTE] = data_format | (endian << ENDIAN_SHIFT);
    TextureHeader::from_bytes(header)
}

#[test]
fn decode_kind_keeps_unsupported_entry_bytes() {
    let tiled = tiled_texture();
    let mut source = AssetFile::new();
    source.set(AssetSlot::Cover, Some(tiled.clone()));

    let mut decoded = decode_kind(&source, AssetKind::Cover);

    assert!(matches!(
        decoded.previews.as_slice(),
        [(AssetSlot::Cover, SlotPreview::Unsupported(_))]
    ));
    assert!(decoded.dropped.is_empty());
    assert_eq!(decoded.file.get(AssetSlot::Cover), Some(&tiled));
    assert_eq!(decoded.file.to_bytes(), source.to_bytes());
}

#[test]
fn decode_kind_drops_undecodable_entry() {
    let mut truncated = encode(&solid(16, 16, [9, 9, 9, 255]), Compression::Dxt).unwrap();
    truncated.data.truncate(4);
    let mut source = AssetFile::new();
    source.set(shot(2), Some(truncated));
    source.set(
        shot(3),
        Some(encode(&solid(8, 8, [1, 1, 1, 255]), Compression::None).unwrap()),
    );

    let decoded = decode_kind(&source, AssetKind::Screenshots);

    assert!(decoded.file.get(shot(2)).is_none());
    assert!(decoded.file.get(shot(3)).is_some());
    assert_eq!(decoded.dropped.len(), 1);
    assert_eq!(decoded.dropped[0].slot, shot(2));
    assert!(matches!(
        decoded.dropped[0].error,
        DecodeError::TruncatedData { .. }
    ));
    assert_eq!(decoded.previews.len(), 1);
    assert_eq!(decoded.previews[0].0, shot(3));
}

#[test]
fn loaded_preview_matches_dll_decode() {
    let cover = placeholder(ImageTarget::Cover);
    let source = asset_file_with(&[(AssetSlot::Cover, cover, Compression::Dxt)]);

    let decoded = decode_kind(&source, AssetKind::Cover);

    let expected = decode(source.get(AssetSlot::Cover).unwrap()).unwrap();
    assert_eq!(
        decoded.previews,
        vec![(AssetSlot::Cover, SlotPreview::Pixels(expected))]
    );
}

#[test]
fn prepared_image_encodes_after_resize() {
    let source = placeholder(ImageTarget::Screenshot);
    let resized = fit_to_target(source.clone(), ImageTarget::Cover, true);
    assert_eq!(resized.dimensions(), (900, 600));

    let prepared =
        PreparedImage::prepare(source, ImageTarget::Cover, true, Compression::Dxt).unwrap();

    assert_eq!(prepared.pixels, resized);
    assert_eq!(
        prepared.texture,
        encode(&encoder_pixels(&resized), Compression::Dxt).unwrap()
    );
}

#[test]
fn prepared_image_encodes_the_gdiplus_copy() {
    let mut source = solid(8, 8, [10, 20, 30, 255]);
    source.put_pixel(1, 1, Rgba([200, 100, 50, 77]));
    source.put_pixel(2, 2, Rgba([255, 0, 0, 0]));

    let results = [
        PreparedImage::prepare(source.clone(), ImageTarget::Icon, false, Compression::None)
            .unwrap(),
        PreparedImage::without_resize(source, Compression::None).unwrap(),
    ];

    for prepared in results {
        assert_eq!(prepared.pixels.get_pixel(1, 1), &Rgba([200, 100, 50, 77]));
        assert_eq!(prepared.pixels.get_pixel(2, 2), &Rgba([255, 0, 0, 0]));
        assert_eq!(
            prepared.texture,
            encode(&encoder_pixels(&prepared.pixels), Compression::None).unwrap()
        );
        assert_ne!(
            prepared.texture,
            encode(&prepared.pixels, Compression::None).unwrap()
        );
    }
}

#[test]
fn set_prepared_shows_source_pixels_not_the_decode() {
    let mut source = solid(16, 16, [10, 20, 30, 255]);
    source.put_pixel(3, 3, Rgba([201, 7, 99, 255]));
    let prepared = PreparedImage::without_resize(source.clone(), Compression::Dxt).unwrap();
    assert_ne!(
        decode(&prepared.texture).unwrap(),
        source,
        "DXT is lossy, so the decode differs from the source and the check below means something"
    );

    let mut document = AssetDocument::new(AssetKind::Cover);
    document.set_prepared(AssetSlot::Cover, prepared);

    assert_eq!(
        document.preview(AssetSlot::Cover),
        &SlotPreview::Pixels(source)
    );
    assert!(document.has_content());
    assert!(document.texture_header(AssetSlot::Cover).is_some());
}

#[test]
fn remove_and_replace_with_update_previews_and_revisions() {
    let mut document = AssetDocument::new(AssetKind::IconBanner);
    document.set_prepared(AssetSlot::Icon, prepared(8, 8, [1, 2, 3, 255]));
    let after_set = document.revision(AssetSlot::Icon);
    assert!(after_set > 0);

    document.remove(AssetSlot::Icon);
    assert_eq!(document.preview(AssetSlot::Icon), &SlotPreview::Empty);
    assert!(!document.has_content());
    assert!(document.revision(AssetSlot::Icon) > after_set);

    let source = asset_file_with(&[(
        AssetSlot::Banner,
        placeholder(ImageTarget::Banner),
        Compression::Dxt,
    )]);
    document.replace_with(decode_kind(&source, AssetKind::IconBanner));
    assert!(matches!(
        document.preview(AssetSlot::Banner),
        SlotPreview::Pixels(_)
    ));
    assert_eq!(document.preview(AssetSlot::Icon), &SlotPreview::Empty);
    assert!(document.has_content());
}

#[test]
fn moving_a_screenshot_moves_pixels_and_bytes() {
    let mut document = screenshot_document(&[(1, RED), (2, GREEN), (3, BLUE)]);
    let red_texture = stored_texture(&mut document, shot(1));
    let blue_texture = stored_texture(&mut document, shot(3));
    let revision_before = document.revision(shot(1));

    document.move_screenshot(number(3), number(1));

    assert_eq!(pixels_of(&document, shot(1)), solid(8, 8, BLUE));
    assert_eq!(pixels_of(&document, shot(2)), solid(8, 8, RED));
    assert_eq!(pixels_of(&document, shot(3)), solid(8, 8, GREEN));
    assert_eq!(stored_texture(&mut document, shot(1)), blue_texture);
    assert_eq!(stored_texture(&mut document, shot(2)), red_texture);
    assert!(document.revision(shot(1)) > revision_before);

    let revision_before_noop = document.revision(shot(10));
    document.move_screenshot(number(10), number(1));
    assert_eq!(document.preview(shot(10)), &SlotPreview::Empty);
    assert_eq!(document.revision(shot(10)), revision_before_noop);
}

#[test]
fn compaction_moves_bytes_and_pixels_without_reencoding() {
    let mut document = screenshot_document(&[(2, RED), (5, GREEN), (9, BLUE)]);
    let before = [2, 5, 9].map(|position| stored_texture(&mut document, shot(position)));
    let revision_before = document.revision(shot(1));

    document.compact_screenshots();

    let after = [1, 2, 3].map(|position| stored_texture(&mut document, shot(position)));
    assert_eq!(after, before);
    assert_eq!(pixels_of(&document, shot(1)), solid(8, 8, RED));
    assert_eq!(pixels_of(&document, shot(2)), solid(8, 8, GREEN));
    assert_eq!(pixels_of(&document, shot(3)), solid(8, 8, BLUE));
    assert_eq!(document.preview(shot(4)), &SlotPreview::Empty);
    assert_eq!(document.screenshot_count(), 3);
    assert_eq!(document.free_screenshots().len(), 17);
    assert_eq!(document.free_screenshots()[0], number(4));
    assert!(document.revision(shot(1)) > revision_before);
}

#[test]
fn present_types_counts_icon_and_banner_separately() {
    let one_pixel = || prepared(8, 8, [1, 1, 1, 255]);
    let mut pages = EditorPages::new();
    assert_eq!(pages.present_types(), 0);

    pages
        .get_mut(AssetKind::IconBanner)
        .set_prepared(AssetSlot::Icon, one_pixel());
    assert_eq!(pages.present_types(), 1);
    pages
        .get_mut(AssetKind::IconBanner)
        .set_prepared(AssetSlot::Banner, one_pixel());
    assert_eq!(pages.present_types(), 2);

    pages
        .get_mut(AssetKind::Cover)
        .set_prepared(AssetSlot::Cover, one_pixel());
    pages
        .get_mut(AssetKind::Background)
        .set_prepared(AssetSlot::Background, one_pixel());
    pages
        .get_mut(AssetKind::Screenshots)
        .set_prepared(shot(4), one_pixel());
    assert_eq!(pages.present_types(), 5);
    assert_eq!(pages.get(AssetKind::Cover).kind(), AssetKind::Cover);

    pages.reset();
    assert_eq!(pages.present_types(), 0);
}

#[test]
fn texture_caption_names_format() {
    let encoded_caption = |width, height, compression| {
        let texture = encode(&solid(width, height, [1, 2, 3, 255]), compression).unwrap();
        texture_caption(&texture.header)
    };
    assert_eq!(
        encoded_caption(900, 600, Compression::Dxt),
        "900 x 600 · DXT"
    );
    assert_eq!(encoded_caption(64, 64, Compression::None), "64 x 64 · ARGB");
    assert_eq!(
        texture_caption(&dxt_header_with(DXT1_DATA_FORMAT, ENDIAN_8IN16)),
        "8 x 8 · DXT1"
    );
    assert_eq!(
        texture_caption(&dxt_header_with(DXT4_5_DATA_FORMAT, ENDIAN_8IN32)),
        "8 x 8 · DXT5"
    );
    assert_eq!(
        texture_caption(&tiled_texture().header),
        "8 x 8 · Unsupported texture format (kept as is)"
    );
}
