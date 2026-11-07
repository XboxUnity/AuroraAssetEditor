use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;

use aurora_asset::{AssetFile, AssetSlot, Compression, TextureHeader, encode};
use aurora_asset_editor::app::CurrentGame;
use aurora_console::{FtpSettings, GameIds, HexId};
use editor_test_support::{AppHarness, Scene, asset_file_with};
use image::{ImageFormat, Rgba, RgbaImage};
use tempfile::TempDir;

const HALO_3_TITLE: &str = "Halo 3";
const HALO_3_TITLE_ID: &str = "4D5307E6";
const HALO_3_DATABASE_ID: i64 = 1;
/// A fixed address on the test subnet: the console is never contacted, and a live test server's
/// address would differ from run to run on screen.
const CONSOLE_HOST: &str = "192.168.1.50";
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

/// A harness whose app starts with the console set up, with the default user and password.
pub fn console_harness() -> AppHarness {
    let mut harness = AppHarness::new();
    harness.with_ftp_settings(FtpSettings {
        host: CONSOLE_HOST.to_owned(),
        port: CONSOLE_PORT.to_owned(),
        ..FtpSettings::default()
    });
    harness
}

const COVER_SIZE: (u32, u32) = (900, 600);
const BACKGROUND_SIZE: (u32, u32) = (1280, 720);
const ICON_SIZE: (u32, u32) = (64, 64);
const BANNER_SIZE: (u32, u32) = (420, 96);
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
/// The gradient moves in this many diagonal bands: a smooth ramp scaled into the slot is noisy enough
/// to push a reference PNG over its size budget, bands are not.
const GRADIENT_BANDS: u32 = 12;
/// How much lighter the top-left quarter is, so a flipped or rotated image shows.
const LIGHT_QUARTER_BOOST: u8 = 48;

/// The asset files the asset page screens load.
#[derive(Clone, Copy)]
pub enum SampleAsset {
    Cover,
    Background,
    IconBanner,
    IconOnly,
    TiledCover,
}

/// A banded diagonal gradient between two colours derived from `seed`, the top-left quarter lighter. Integer
/// arithmetic only, so the pixels are the same on every machine.
pub fn sample_art(width: u32, height: u32, seed: u8) -> RgbaImage {
    let start = [
        base_channel(seed, 67, 40),
        base_channel(seed, 113, 90),
        base_channel(seed, 29, 160),
    ];
    let end = start.map(|channel| u8::MAX - channel);
    RgbaImage::from_fn(width, height, |x, y| {
        let smooth = (x * FULL_CHANNEL / (width - 1) + y * FULL_CHANNEL / (height - 1)) / 2;
        let along =
            smooth * GRADIENT_BANDS / (FULL_CHANNEL + 1) * FULL_CHANNEL / (GRADIENT_BANDS - 1);
        let in_light_quarter = x < width / 2 && y < height / 2;
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
        Rgba([channel(0), channel(1), channel(2), u8::MAX])
    })
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

/// A cover whose texture carries the tiled flag.
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

/// Opens an `.asset` file with the given name and bytes and runs the load.
pub fn load_asset_file(scene: &mut Scene, file_name: &str, bytes: &[u8]) {
    let path = write_to_scene_folder(scene, file_name, bytes);
    scene.act(|app| app.open_paths(vec![path]));
    scene.run_jobs();
}

/// Writes `image` as a PNG named `file_name`, for a dialog or a drop to hand to the app.
pub fn png_file(scene: &mut Scene, file_name: &str, image: &RgbaImage) -> PathBuf {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("a sample image encodes as PNG");
    write_to_scene_folder(scene, file_name, bytes.get_ref())
}

/// A file in a folder that lives as long as the scene.
fn write_to_scene_folder(scene: &mut Scene, file_name: &str, bytes: &[u8]) -> PathBuf {
    let folder = TempDir::new().expect("the scene needs a temp folder");
    let path = folder.path().join(file_name);
    fs::write(&path, bytes).expect("the sample file can be written");
    scene.keep_alive(folder);
    path
}
