use std::fs;
use std::io::Cursor;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::OnceLock;

use aurora_asset::{AssetFile, AssetSlot, Compression, EncodedTexture, TextureHeader, encode};
use aurora_asset_editor::app::CurrentGame;
use aurora_console::{FtpSettings, Game, GameIds, HexId};
use editor_test_support::{AppHarness, FsdBuilder, STAGED_CONSOLE_HOST, Scene, asset_file_with};
use image::{ImageFormat, Rgba, RgbaImage};
use tempfile::TempDir;

const HALO_3_TITLE: &str = "Halo 3";
const HALO_3_TITLE_ID: &str = "4D5307E6";
const HALO_3_DATABASE_ID: i64 = 1;
const CONSOLE_PORT: &str = "21";

/// The game the catalogue's "game" means.
pub fn halo_3() -> CurrentGame {
    CurrentGame {
        title: HALO_3_TITLE.to_owned(),
        ids: GameIds {
            title_id: HexId::parse(HALO_3_TITLE_ID).expect("the sample TitleID is valid"),
            database_id: HexId::from_database_value(HALO_3_DATABASE_ID),
        },
    }
}

/// The console's games as the catalogue lists them, in the order a refresh returns them.
pub fn sample_games() -> Vec<Game> {
    SAMPLE_GAMES
        .iter()
        .map(|sample| Game {
            title_name: sample.title.to_owned(),
            ids: GameIds {
                title_id: HexId::parse(sample.title_id).expect("the sample TitleID is valid"),
                database_id: HexId::from_database_value(sample.database_id),
            },
            media_id: HexId::parse(SAMPLE_MEDIA_ID).expect("the sample MediaID is valid"),
            disc: sample.disc,
        })
        .collect()
}

/// A harness whose app starts with the console set up, with the default user and password.
pub fn console_harness() -> AppHarness {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: STAGED_CONSOLE_HOST.to_owned(),
        port: CONSOLE_PORT.to_owned(),
        ..FtpSettings::default()
    });
    harness
}

const SAMPLE_MEDIA_ID: &str = "1A2B3C4D";

struct SampleGame {
    title: &'static str,
    disc: u32,
    title_id: &'static str,
    database_id: i64,
}

const SAMPLE_GAMES: [SampleGame; 8] = [
    SampleGame {
        title: "Halo 3",
        disc: 1,
        title_id: "4D5307E6",
        database_id: 1,
    },
    SampleGame {
        title: "Fable II",
        disc: 2,
        title_id: "4D530842",
        database_id: 2,
    },
    SampleGame {
        title: "Hexic HD",
        disc: 1,
        title_id: "58410A1C",
        database_id: 18,
    },
    SampleGame {
        title: "Gears of War 2",
        disc: 1,
        title_id: "4D53082D",
        database_id: 3,
    },
    SampleGame {
        title: "Forza Motorsport 3",
        disc: 1,
        title_id: "4D530896",
        database_id: 4,
    },
    SampleGame {
        title: "Mass Effect",
        disc: 1,
        title_id: "4D5307EF",
        database_id: 5,
    },
    SampleGame {
        title: "Banjo-Kazooie",
        disc: 1,
        title_id: "584108A9",
        database_id: 6,
    },
    SampleGame {
        title: "Perfect Dark",
        disc: 1,
        title_id: "584108A8",
        database_id: 7,
    },
];

const COVER_SIZE: (u32, u32) = (900, 600);
const BACKGROUND_SIZE: (u32, u32) = (1280, 720);
const ICON_SIZE: (u32, u32) = (64, 64);
const BANNER_SIZE: (u32, u32) = (420, 96);
const SCREENSHOT_SIZE: (u32, u32) = (1000, 562);
const SCREENSHOT_SEED_BASE: u8 = 10;
const TILED_COVER_SIZE: (u32, u32) = (8, 8);
const COVER_SEED: u8 = 1;
const BACKGROUND_SEED: u8 = 2;
const ICON_SEED: u8 = 3;
const BANNER_SEED: u8 = 4;
/// A fixed byte of the texture header holds the tiled flag; a tiled texture is one the editor cannot
/// decode.
const TILED_FLAG_BYTE: usize = 7 * 4;
const TILED_FLAG: u8 = 0x80;
const FULL_CHANNEL: u32 = 255;
const MIN_BASE_CHANNEL: u8 = 48;
const BASE_CHANNEL_RANGE: u8 = 160;
/// The gradient moves in this many bands from left to right. A smooth ramp, or bands running diagonally,
/// resized smoothly into a slot (and DXT blocks along their edges) differ from row to row and make every
/// screenshot much larger; bands running straight down repeat row after row and stay readable in a
/// thumbnail.
const GRADIENT_BANDS: u32 = 12;
/// How much lighter the top-left quarter is, so a flipped or rotated image shows.
const LIGHT_QUARTER_BOOST: u8 = 48;
/// DXT keeps red and blue in 5 bits and green in 6, so a channel holds 32 or 64 levels; a colour on those
/// levels needs no dithering between two of them.
const RED_BLUE_STEPS: u32 = 31;
const GREEN_STEPS: u32 = 63;
/// DXT stores the pixels in blocks of 4x4.
const DXT_BLOCK_SIDE: u32 = 4;
/// Even in a flat block, the encoder stores a green level in the middle of each third of the range as a
/// pattern of two neighbouring levels; the art leaves those levels out.
const GREEN_THIRD: u32 = 21;
const SPLIT_GREEN_STEPS: RangeInclusive<u32> = 9..=12;
/// Split levels up to here move down to the level below the range, the others up to the one above.
const SPLIT_GREEN_MIDDLE: u32 = 10;

const TEXT_FILE_CONTENT: &[u8] = b"These are notes, not an asset file.";
const TRUNCATED_ENTRY: u8 = 2;
const SOUND_ENTRY: u8 = 3;
const SOUND_ENTRY_SEED: u8 = 6;
const ENTRY_SIZE: (u32, u32) = (16, 16);
const TRUNCATED_SIZE: (u32, u32) = (16, 16);
const TRUNCATED_SEED: u8 = 9;
/// Fewer bytes than the texture's pixels need, so decoding it fails.
const TRUNCATED_DATA_LEN: usize = 4;
const FSD_SCREENSHOT_SIZE: (u32, u32) = (20, 12);
const FSD_SCREENSHOT_SEED: u8 = 8;

/// The asset files the asset page screens load.
#[derive(Clone, Copy)]
pub enum SampleAsset {
    Cover,
    Background,
    IconBanner,
    IconOnly,
    TiledCover,
}

/// A banded gradient from left to right between two colours derived from `seed`, the top-left quarter
/// (the left half of the bands, the upper half of the rows) lighter. Integer arithmetic only, so the
/// pixels are the same on every machine. Bands and quarter follow DXT's 4x4 blocks and every colour is
/// one DXT stores exactly, so each block of a DXT sample is one exact colour and decodes to the same
/// flat bands.
pub fn sample_art(width: u32, height: u32, seed: u8) -> RgbaImage {
    let start = [
        base_channel(seed, 67, 40),
        base_channel(seed, 113, 90),
        base_channel(seed, 29, 160),
    ];
    let end = start.map(|channel| u8::MAX - channel);
    let block_columns = width.div_ceil(DXT_BLOCK_SIDE);
    let light_rows = height / 2 / DXT_BLOCK_SIDE * DXT_BLOCK_SIDE;
    RgbaImage::from_fn(width, height, |x, y| {
        let band = x / DXT_BLOCK_SIDE * GRADIENT_BANDS / block_columns;
        let along = band * FULL_CHANNEL / (GRADIENT_BANDS - 1);
        let in_light_quarter = band < GRADIENT_BANDS / 2 && y < light_rows;
        let channel = |index: usize| {
            let mixed = (u32::from(start[index]) * (FULL_CHANNEL - along)
                + u32::from(end[index]) * along)
                / FULL_CHANNEL;
            let mixed = u8::try_from(mixed).expect("a mix of two channels fits a channel");
            if in_light_quarter {
                mixed.saturating_add(LIGHT_QUARTER_BOOST)
            } else {
                mixed
            }
        };
        Rgba([
            on_dxt_step(channel(0), RED_BLUE_STEPS),
            green_on_dxt_step(channel(1)),
            on_dxt_step(channel(2), RED_BLUE_STEPS),
            u8::MAX,
        ])
    })
}

/// The nearest of the values a DXT colour channel with `steps + 1` levels can hold.
fn on_dxt_step(value: u8, steps: u32) -> u8 {
    let step = (u32::from(value) * steps + FULL_CHANNEL / 2) / FULL_CHANNEL;
    dxt_step_value(step, steps)
}

/// Like `on_dxt_step`, skipping the green steps the encoder splits.
fn green_on_dxt_step(value: u8) -> u8 {
    let step = (u32::from(value) * GREEN_STEPS + FULL_CHANNEL / 2) / FULL_CHANNEL;
    let within_third = step % GREEN_THIRD;
    let step = if SPLIT_GREEN_STEPS.contains(&within_third) {
        let third_start = step - within_third;
        if within_third <= SPLIT_GREEN_MIDDLE {
            third_start + SPLIT_GREEN_STEPS.start() - 1
        } else {
            third_start + SPLIT_GREEN_STEPS.end() + 1
        }
    } else {
        step
    };
    dxt_step_value(step, GREEN_STEPS)
}

/// The channel value the decoder gives step `step` of `steps`.
fn dxt_step_value(step: u32, steps: u32) -> u8 {
    u8::try_from((step * FULL_CHANNEL + steps / 2) / steps)
        .expect("a step is at most a full channel")
}

/// A channel of a seed's start colour, kept clear of black and white so every seed reads as colour
/// on both themes.
fn base_channel(seed: u8, multiplier: u8, offset: u8) -> u8 {
    let spread = seed.wrapping_mul(multiplier).wrapping_add(offset);
    MIN_BASE_CHANNEL + spread % BASE_CHANNEL_RANGE
}

/// The bytes of a sample `.asset` file. The DXT encoder is slow in a debug build and many screens load
/// these files, so each file is encoded once per process.
pub fn sample_asset_bytes(asset: SampleAsset) -> Vec<u8> {
    static COVER: OnceLock<Vec<u8>> = OnceLock::new();
    static ICON_BANNER: OnceLock<Vec<u8>> = OnceLock::new();
    static ICON_ONLY: OnceLock<Vec<u8>> = OnceLock::new();
    static TILED_COVER: OnceLock<Vec<u8>> = OnceLock::new();
    match asset {
        SampleAsset::Cover => COVER.get_or_init(cover_bytes).clone(),
        SampleAsset::Background => background_bytes(),
        SampleAsset::IconBanner => ICON_BANNER.get_or_init(icon_banner_bytes).clone(),
        SampleAsset::IconOnly => ICON_ONLY.get_or_init(icon_only_bytes).clone(),
        SampleAsset::TiledCover => TILED_COVER.get_or_init(tiled_cover_bytes).clone(),
    }
}

/// An uncompressed screenshots file holding the given positions, each in its own colours so the
/// strip shows which thumbnail is which.
pub fn sample_screenshots_bytes(positions: &[u8]) -> Vec<u8> {
    let entries: Vec<_> = positions
        .iter()
        .map(|position| {
            let slot = AssetSlot::screenshot(*position).expect("a screenshot position");
            art_entry(
                slot,
                SCREENSHOT_SIZE,
                SCREENSHOT_SEED_BASE + position,
                Compression::None,
            )
        })
        .collect();
    asset_file_with(&entries).to_bytes()
}

fn art_entry(
    slot: AssetSlot,
    size: (u32, u32),
    seed: u8,
    compression: Compression,
) -> (AssetSlot, RgbaImage, Compression) {
    (slot, sample_art(size.0, size.1, seed), compression)
}

fn cover_bytes() -> Vec<u8> {
    asset_file_with(&[art_entry(
        AssetSlot::Cover,
        COVER_SIZE,
        COVER_SEED,
        Compression::Dxt,
    )])
    .to_bytes()
}

fn background_bytes() -> Vec<u8> {
    asset_file_with(&[art_entry(
        AssetSlot::Background,
        BACKGROUND_SIZE,
        BACKGROUND_SEED,
        Compression::None,
    )])
    .to_bytes()
}

fn icon_banner_bytes() -> Vec<u8> {
    asset_file_with(&[
        art_entry(AssetSlot::Icon, ICON_SIZE, ICON_SEED, Compression::Dxt),
        art_entry(
            AssetSlot::Banner,
            BANNER_SIZE,
            BANNER_SEED,
            Compression::Dxt,
        ),
    ])
    .to_bytes()
}

fn icon_only_bytes() -> Vec<u8> {
    asset_file_with(&[art_entry(
        AssetSlot::Icon,
        ICON_SIZE,
        ICON_SEED,
        Compression::Dxt,
    )])
    .to_bytes()
}

fn tiled_cover_bytes() -> Vec<u8> {
    let art = sample_art(TILED_COVER_SIZE.0, TILED_COVER_SIZE.1, COVER_SEED);
    let mut texture = encode(&art, Compression::Dxt).expect("an 8x8 image encodes");
    let mut header = texture.header.to_bytes();
    header[TILED_FLAG_BYTE] |= TILED_FLAG;
    texture.header = TextureHeader::from_bytes(header);
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture));
    file.to_bytes()
}

pub fn load_asset_file(scene: &mut Scene, file_name: &str, bytes: &[u8]) {
    let path = write_to_scene_folder(scene, file_name, bytes);
    scene.act(|app| app.open_paths(vec![path]));
    scene.run_jobs();
}

/// Writes `image` as a PNG named `file_name`, for a dialog or a drop to hand to the app.
pub fn png_file(scene: &mut Scene, file_name: &str, image: &RgbaImage) -> PathBuf {
    write_to_scene_folder(scene, file_name, &png_bytes(image))
}

pub fn png_bytes(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("a sample image encodes as PNG");
    bytes.into_inner()
}

/// A file in a folder that lives as long as the scene.
fn write_to_scene_folder(scene: &mut Scene, file_name: &str, bytes: &[u8]) -> PathBuf {
    let folder = TempDir::new().expect("the scene needs a temp folder");
    let path = folder.path().join(file_name);
    fs::write(&path, bytes).expect("the sample file can be written");
    scene.keep_alive(folder);
    path
}

/// A text file named `file_name`: not an image, not an `.asset` file, whatever its name says.
pub fn text_file(scene: &mut Scene, file_name: &str) -> PathBuf {
    write_to_scene_folder(scene, file_name, TEXT_FILE_CONTENT)
}

/// A screenshots file whose entry 2 has a texture too short for its header and whose entry 3 is
/// sound, so loading drops the first and reports it.
pub fn undecodable_entry_asset() -> Vec<u8> {
    let truncated_slot = AssetSlot::screenshot(TRUNCATED_ENTRY).expect("a screenshot position");
    let sound_slot = AssetSlot::screenshot(SOUND_ENTRY).expect("a screenshot position");
    let mut file = AssetFile::new();
    file.set(truncated_slot, Some(truncated_texture()));
    file.set(
        sound_slot,
        Some(
            encode(
                &sample_art(ENTRY_SIZE.0, ENTRY_SIZE.1, SOUND_ENTRY_SEED),
                Compression::None,
            )
            .expect("a small image encodes"),
        ),
    );
    file.to_bytes()
}

pub fn fsd_with_screenshots(count: usize) -> Vec<u8> {
    let screenshot = png_bytes(&sample_art(
        FSD_SCREENSHOT_SIZE.0,
        FSD_SCREENSHOT_SIZE.1,
        FSD_SCREENSHOT_SEED,
    ));
    (0..count)
        .fold(FsdBuilder::new(), |builder, _| {
            builder.screenshot(&screenshot)
        })
        .build()
}

fn truncated_texture() -> EncodedTexture {
    let mut texture = encode(
        &sample_art(TRUNCATED_SIZE.0, TRUNCATED_SIZE.1, TRUNCATED_SEED),
        Compression::Dxt,
    )
    .expect("a small image encodes");
    texture.data.truncate(TRUNCATED_DATA_LEN);
    texture
}

#[test]
fn dxt_samples_decode_to_the_art_they_were_made_from() {
    let samples = [
        (COVER_SIZE, COVER_SEED),
        (ICON_SIZE, ICON_SEED),
        (BANNER_SIZE, BANNER_SEED),
    ];
    for ((width, height), seed) in samples {
        let art = sample_art(width, height, seed);
        let texture = encode(&art, Compression::Dxt).expect("the sample encodes");
        let decoded = aurora_asset::decode(&texture).expect("the sample decodes");
        assert!(
            decoded == art,
            "the {width}x{height} sample of seed {seed} changed in DXT"
        );
    }
}
