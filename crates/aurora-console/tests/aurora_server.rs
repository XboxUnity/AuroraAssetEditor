//! A full session against the fake console in its default mode, which answers like Aurora's FtpDll.

use std::path::PathBuf;
use std::time::Duration;

use aurora_asset::{
    AssetFile, AssetKind, AssetSlot, Compression, EncodedTexture, RgbaImage, encode,
};
use aurora_console::{
    AssetPresence, ConsoleClient, ConsoleError, FtpSettings, GameIds, HexId, Timeouts,
    read_content_db,
};
use test_servers::{FakeConsole, FakeConsoleConfig};

const SHORT: Duration = Duration::from_millis(500);
const GAME_FOLDER: &str = "/Game/Data/GameData/4D5307E6_00000012";
const MAGIC: u32 = 0x5258_4541;
const ASSET_PACK_VERSION: u32 = 1;
/// `ASSET_MAX` entries of offset, size, extended-info pointer and the 0x34-byte texture header.
const ASSET_ENTRIES: usize = 25;
const ASSET_ENTRY_LEN: usize = 4 + 4 + 4 + 0x34;
/// Flags and screenshot count, then the entries, then the table's 0x1AC bytes of padding.
const ASSET_TABLE_LEN: usize = 4 + 4 + ASSET_ENTRIES * ASSET_ENTRY_LEN + 0x1AC;

fn client_for(console: &FakeConsole) -> ConsoleClient {
    ConsoleClient::new(
        FtpSettings {
            host: "127.0.0.1".to_owned(),
            port: console.port().to_string(),
            username: "xboxftp".to_owned(),
            password: "xboxftp".to_owned(),
        },
        Timeouts {
            connect: SHORT,
            read: SHORT,
        },
        |_| {},
    )
}

fn game() -> GameIds {
    GameIds {
        title_id: HexId::parse("4D5307E6").unwrap(),
        database_id: HexId::parse("00000012").unwrap(),
    }
}

/// What Aurora's `CreateAssetPack` writes: the header (magic, version 1, data size 0) and a zeroed
/// entry table, nothing else (`ContentAssetPack.cpp:109-136`).
fn aurora_empty_pack() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC.to_be_bytes());
    bytes.extend_from_slice(&ASSET_PACK_VERSION.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.resize(12 + ASSET_TABLE_LEN, 0);
    bytes
}

fn add_aurora_empty_packs(console: &FakeConsole) {
    for prefix in ["GL", "GC", "BK", "SS"] {
        console.add_file(
            &format!("{GAME_FOLDER}/{prefix}4D5307E6.asset"),
            &aurora_empty_pack(),
        );
    }
}

fn small_texture(shade: u8) -> EncodedTexture {
    let image = RgbaImage::from_pixel(8, 8, image::Rgba([shade, 40, 90, 255]));
    encode(&image, Compression::None).unwrap()
}

fn cover_asset(shade: u8) -> AssetFile {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(small_texture(shade)));
    file
}

#[test]
fn test_connection_reports_the_aurora_revision() {
    let console = FakeConsole::start(FakeConsoleConfig::default());

    let revision = client_for(&console).test_connection().unwrap();

    assert_eq!(revision.as_str(), "1655");
}

#[test]
fn lists_games_from_the_content_db() {
    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/console/content-db/typical.db");
    let content_db = std::fs::read(&sample).expect("the sample database is readable");
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &content_db);
    let temp_dir = tempfile::tempdir().expect("a temp dir is created");

    let listing = client_for(&console)
        .list_games_in(temp_dir.path())
        .expect("the games are listed");

    assert_eq!(
        listing.games,
        read_content_db(&sample).expect("the sample database reads")
    );
}

#[test]
fn aurora_empty_packs_count_as_absent() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    add_aurora_empty_packs(&console);
    let mut client = client_for(&console);

    assert_eq!(client.asset_presence(&game()).unwrap(), AssetPresence::NONE);

    let cover = cover_asset(200).to_bytes();
    console.add_file(&format!("{GAME_FOLDER}/GC4D5307E6.asset"), &cover);
    let presence = client.asset_presence(&game()).unwrap();
    assert!(presence.has(AssetKind::Cover));
    assert!(!presence.has(AssetKind::Background));
    assert!(!presence.has(AssetKind::IconBanner));
    assert!(!presence.has(AssetKind::Screenshots));
}

#[test]
fn download_upload_and_remove_round_trip() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    add_aurora_empty_packs(&console);
    let mut client = client_for(&console);
    let cover_path = format!("{GAME_FOLDER}/GC4D5307E6.asset");

    let empty = client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap()
        .expect("Aurora's empty pack is a file with no textures");
    assert_eq!(empty.get(AssetSlot::Cover), None);

    let mut uploaded = cover_asset(200);
    client
        .upload_asset(&game(), AssetKind::Cover, &uploaded.to_bytes())
        .unwrap();
    assert_eq!(console.file(&cover_path), Some(uploaded.to_bytes()));
    let downloaded = client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap()
        .unwrap();
    assert_eq!(downloaded.get(AssetSlot::Cover), Some(&small_texture(200)));
    assert!(
        client
            .asset_presence(&game())
            .unwrap()
            .has(AssetKind::Cover)
    );

    client.remove_asset(&game(), AssetKind::Cover).unwrap();
    assert_eq!(console.file(&cover_path), Some(aurora_empty_pack()));
    assert!(
        !client
            .asset_presence(&game())
            .unwrap()
            .has(AssetKind::Cover)
    );
}

#[test]
fn a_425_from_the_console_is_a_typed_error() {
    let console = FakeConsole::start(FakeConsoleConfig {
        data_connect_timeout: Duration::from_millis(100),
        ..FakeConsoleConfig::default()
    });
    add_aurora_empty_packs(&console);
    console.refuse_data_connections();

    let error = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(
        matches!(&error, ConsoleError::DataConnectionFailed { action } if action.starts_with("list ")),
        "{error:?}"
    );
    assert!(error.to_string().contains("data connection"), "{error}");
}
