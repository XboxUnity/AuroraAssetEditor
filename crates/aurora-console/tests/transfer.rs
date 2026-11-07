use std::io;
use std::time::{Duration, Instant};

use aurora_asset::{
    AssetFile, AssetKind, AssetSlot, Compression, EncodedTexture, RgbaImage, encode,
};
use aurora_console::{
    ConsoleClient, ConsoleError, ConsoleStatus, FtpSettings, GameIds, HexId, Timeouts,
};
use suppaftp::FtpError;
use test_servers::{FakeConsole, FakeConsoleConfig, FakeFeatures, ListingStyle};

const HOST: &str = "127.0.0.1";
const SHORT: Duration = Duration::from_millis(500);
const FOLDER: &str = "/Game/Data/GameData/4D5307E6_00000012";
const COVER_FILE: &str = "/Game/Data/GameData/4D5307E6_00000012/GC4D5307E6.asset";
const EMPTY_ASSET_LEN: usize = 2048;
const MEBIBYTE: u64 = 1024 * 1024;

fn client_for(console: &FakeConsole) -> ConsoleClient {
    ConsoleClient::new(
        FtpSettings {
            host: HOST.to_owned(),
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

fn ids(title_id: &str, database_id: &str) -> GameIds {
    GameIds {
        title_id: HexId::parse(title_id).unwrap(),
        database_id: HexId::parse(database_id).unwrap(),
    }
}

fn game() -> GameIds {
    ids("4D5307E6", "00000012")
}

fn console_with_game_folder(config: FakeConsoleConfig) -> FakeConsole {
    let console = FakeConsole::start(config);
    console.add_dir(FOLDER);
    console
}

fn small_texture(shade: u8) -> EncodedTexture {
    let image = RgbaImage::from_pixel(8, 8, image::Rgba([shade, 40, 90, 255]));
    encode(&image, Compression::None).unwrap()
}

fn cover_asset_bytes(shade: u8) -> Vec<u8> {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(small_texture(shade)));
    file.to_bytes()
}

fn empty_asset_bytes() -> Vec<u8> {
    AssetFile::new().to_bytes()
}

fn commands_containing(console: &FakeConsole, verb: &str) -> Vec<String> {
    console
        .commands()
        .into_iter()
        .filter(|command| command.split(' ').next() == Some(verb))
        .collect()
}

fn position_of(commands: &[String], command: &str) -> usize {
    commands
        .iter()
        .position(|logged| logged == command)
        .unwrap_or_else(|| panic!("{command} was not sent: {commands:?}"))
}

#[test]
fn downloads_and_parses_cover() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let bytes = cover_asset_bytes(200);
    console.add_file(COVER_FILE, &bytes);

    let asset = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap()
        .unwrap();

    assert_eq!(asset.get(AssetSlot::Cover), Some(&small_texture(200)));
    let commands = console.commands();
    let change = position_of(&commands, &format!("CWD {FOLDER}"));
    let list = position_of(&commands, &format!("LIST {FOLDER}"));
    let retrieve = position_of(&commands, "RETR GC4D5307E6.asset");
    assert!(change < list && list < retrieve, "{commands:?}");
}

#[test]
fn uses_mlsd_when_advertised() {
    let console = console_with_game_folder(FakeConsoleConfig {
        features: FakeFeatures::MlsdAndUtf8,
        listing_style: ListingStyle::Mlsd,
        ..FakeConsoleConfig::default()
    });
    console.add_file(COVER_FILE, &cover_asset_bytes(200));

    let asset = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap();

    assert!(asset.is_some());
    assert!(console.commands().contains(&format!("MLSD {FOLDER}")));
    assert!(commands_containing(&console, "LIST").is_empty());
}

#[test]
fn listing_match_is_case_insensitive() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(
        "/Game/Data/GameData/4D5307E6_00000012/gc4d5307e6.ASSET",
        &cover_asset_bytes(200),
    );

    let asset = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap();

    assert!(asset.is_some());
    assert!(
        console
            .commands()
            .contains(&"RETR gc4d5307e6.ASSET".to_owned())
    );
}

#[test]
fn missing_asset_file_is_none() {
    let console = console_with_game_folder(FakeConsoleConfig::default());

    let asset = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap();

    assert!(asset.is_none());
    assert!(commands_containing(&console, "RETR").is_empty());
}

#[test]
fn zero_byte_file_is_none() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &[]);

    let asset = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap();

    assert!(asset.is_none());
    assert!(commands_containing(&console, "RETR").is_empty());
}

#[test]
fn missing_folder_download_is_none() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    assert!(
        client
            .download_asset(&game(), AssetKind::Cover)
            .unwrap()
            .is_none()
    );
    assert!(
        client
            .download_asset(&game(), AssetKind::Cover)
            .unwrap()
            .is_none()
    );

    assert_eq!(console.login_count(), 1);
}

#[test]
fn missing_folder_presence_is_all_false() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    for _ in 0..2 {
        let presence = client.asset_presence(&game()).unwrap();
        assert!(AssetKind::ALL.iter().all(|kind| !presence.has(*kind)));
    }

    assert_eq!(console.login_count(), 1);
}

#[test]
fn missing_folder_upload_is_reported() {
    let console = FakeConsole::start(FakeConsoleConfig::default());

    let error = client_for(&console)
        .upload_asset(&game(), AssetKind::Cover, &cover_asset_bytes(200))
        .unwrap_err();

    assert!(matches!(&error, ConsoleError::MissingFolder { path } if path == FOLDER));
    assert!(console.file(COVER_FILE).is_none());
}

#[test]
fn missing_folder_remove_is_reported() {
    let console = FakeConsole::start(FakeConsoleConfig::default());

    let error = client_for(&console)
        .remove_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(&error, ConsoleError::MissingFolder { path } if path == FOLDER));
    assert!(console.file(COVER_FILE).is_none());
}

#[test]
fn status_texts() {
    assert_eq!(
        ConsoleStatus::OpeningFolder {
            path: "/a".to_owned()
        }
        .to_string(),
        "Opening /a..."
    );
    assert_eq!(
        ConsoleStatus::Downloading {
            file: "GC1.asset".to_owned()
        }
        .to_string(),
        "Downloading GC1.asset..."
    );
    assert_eq!(
        ConsoleStatus::Uploading {
            file: "GC1.asset".to_owned()
        }
        .to_string(),
        "Uploading GC1.asset..."
    );
}

#[test]
fn corrupt_asset_is_invalid_asset() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &[0x41; EMPTY_ASSET_LEN]);

    let error = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(
        &error,
        ConsoleError::InvalidAsset { file, .. } if file == "GC4D5307E6.asset"
    ));
}

#[test]
fn reports_short_read() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &empty_asset_bytes());
    console.truncate_download(COVER_FILE, 100);

    let error = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(
        error,
        ConsoleError::SizeMismatch {
            listed: 2048,
            received: 100,
            ..
        }
    ));
}

#[test]
fn reports_extra_bytes() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &empty_asset_bytes());
    console.list_size_as(COVER_FILE, 1000);

    let error = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(
        &error,
        ConsoleError::SizeMismatch { file, listed: 1000, received: 1001 }
            if file == "GC4D5307E6.asset"
    ));
    assert!(error.to_string().ends_with("more than that"));
}

fn assert_read_timeout(error: &ConsoleError) {
    let ConsoleError::Protocol {
        source: FtpError::ConnectionError(io_error),
        ..
    } = error
    else {
        panic!("expected a timed out read, got {error:?}");
    };
    assert!(
        matches!(
            io_error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ),
        "{io_error:?}"
    );
}

fn assert_fails_after_one_timeout(started: Instant) {
    assert!(
        started.elapsed() < SHORT.mul_f32(1.5),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn stalled_download_times_out() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &empty_asset_bytes());
    console.stall_download(COVER_FILE, 10);
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    let started = Instant::now();

    let error = client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert_read_timeout(&error);
    assert_fails_after_one_timeout(started);
}

#[test]
fn stalled_listing_times_out() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.stall_listing(FOLDER);
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    let started = Instant::now();

    let error = client.asset_presence(&game()).unwrap_err();

    assert_read_timeout(&error);
    assert_fails_after_one_timeout(started);
}

#[test]
fn stalled_mlsd_listing_times_out() {
    let console = console_with_game_folder(FakeConsoleConfig {
        features: FakeFeatures::MlsdAndUtf8,
        listing_style: ListingStyle::Mlsd,
        ..FakeConsoleConfig::default()
    });
    console.stall_listing(FOLDER);
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    let started = Instant::now();

    let error = client.asset_presence(&game()).unwrap_err();

    assert_read_timeout(&error);
    assert_fails_after_one_timeout(started);
}

#[test]
fn rejects_listed_size_above_limit() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &empty_asset_bytes());
    console.list_size_as(COVER_FILE, 300 * MEBIBYTE);

    let error = client_for(&console)
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(
        error,
        ConsoleError::FileTooLarge { size, limit, .. }
            if size == 300 * MEBIBYTE && limit == 256 * MEBIBYTE
    ));
    assert!(commands_containing(&console, "RETR").is_empty());
}

#[test]
fn upload_keeps_bytes_exactly() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut bytes = cover_asset_bytes(200);
    bytes.extend_from_slice(&[0x0A, 0x0D, 0x0A, 0x00, 0x0A]);
    let mut client = client_for(&console);

    client
        .upload_asset(&game(), AssetKind::Cover, &bytes)
        .unwrap();
    client
        .upload_asset(&game(), AssetKind::Cover, &bytes)
        .unwrap();

    assert_eq!(console.file(COVER_FILE), Some(bytes));
    let commands = console.commands();
    let binary_modes: Vec<_> = commands_containing(&console, "TYPE");
    assert_eq!(binary_modes, ["TYPE I"]);
    assert!(position_of(&commands, "TYPE I") < position_of(&commands, "STOR GC4D5307E6.asset"));
}

#[test]
fn upload_replaces_longer_existing_file() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &[0x55; 3000]);
    let bytes = empty_asset_bytes();

    client_for(&console)
        .upload_asset(&game(), AssetKind::Cover, &bytes)
        .unwrap();

    assert_eq!(console.file(COVER_FILE), Some(bytes));
}

#[test]
fn upload_keeps_typed_case() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_dir(FOLDER);

    client_for(&console)
        .upload_asset(
            &ids("4d5307e6", "00000012"),
            AssetKind::Cover,
            &empty_asset_bytes(),
        )
        .unwrap();

    let commands = console.commands();
    assert!(commands.contains(&"CWD /Game/Data/GameData/4d5307e6_00000012".to_owned()));
    assert!(commands.contains(&"STOR GC4d5307e6.asset".to_owned()));
}

#[test]
fn remove_uploads_empty_asset() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &cover_asset_bytes(200));

    client_for(&console)
        .remove_asset(&game(), AssetKind::Cover)
        .unwrap();

    let stored = console.file(COVER_FILE).unwrap();
    assert_eq!(stored, empty_asset_bytes());
    assert_eq!(stored.len(), EMPTY_ASSET_LEN);
    assert_eq!(stored[..8], [0x52, 0x58, 0x45, 0x41, 0, 0, 0, 1]);
    assert!(stored[8..].iter().all(|byte| *byte == 0));
}

#[test]
fn asset_presence_from_one_listing() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &cover_asset_bytes(200));
    console.add_file(
        "/Game/Data/GameData/4D5307E6_00000012/SS4D5307E6.asset",
        &cover_asset_bytes(100),
    );
    console.add_file(
        "/Game/Data/GameData/4D5307E6_00000012/BK4D5307E6.asset",
        &[],
    );

    let presence = client_for(&console).asset_presence(&game()).unwrap();

    assert!(presence.has(AssetKind::Cover));
    assert!(presence.has(AssetKind::Screenshots));
    assert!(!presence.has(AssetKind::Background));
    assert!(!presence.has(AssetKind::IconBanner));
    assert_eq!(commands_containing(&console, "LIST").len(), 1);
}

#[test]
fn asset_presence_treats_empty_asset_as_absent() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    console.add_file(COVER_FILE, &empty_asset_bytes());
    assert!(!client.asset_presence(&game()).unwrap().cover);

    console.add_file(COVER_FILE, &[0; EMPTY_ASSET_LEN + 1]);
    assert!(client.asset_presence(&game()).unwrap().cover);
}

#[test]
fn removed_asset_reads_as_absent() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &cover_asset_bytes(200));
    let mut client = client_for(&console);
    assert!(client.asset_presence(&game()).unwrap().cover);

    client.remove_asset(&game(), AssetKind::Cover).unwrap();

    assert!(!client.asset_presence(&game()).unwrap().cover);
}

#[test]
fn error_drops_connection() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    let mut client = client_for(&console);

    client
        .upload_asset(&game(), AssetKind::Cover, &empty_asset_bytes())
        .unwrap_err();
    client.asset_presence(&game()).unwrap();

    assert_eq!(console.login_count(), 2);
}

#[test]
fn refused_folder_change_is_missing_folder() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.answer_folder_changes_with(Some("550 Access denied"));

    let error = client_for(&console)
        .upload_asset(&game(), AssetKind::Cover, &empty_asset_bytes())
        .unwrap_err();

    assert!(matches!(&error, ConsoleError::MissingFolder { path } if path == FOLDER));
}

#[test]
fn transient_folder_change_refusal_drops_connection() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut client = client_for(&console);
    console.answer_folder_changes_with(Some("421 Service not available"));

    let error = client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap_err();

    assert!(matches!(
        &error,
        ConsoleError::Refused { reply, .. } if reply == "421 Service not available"
    ));
    console.answer_folder_changes_with(None);
    client.asset_presence(&game()).unwrap();
    assert_eq!(console.login_count(), 2);
}

#[test]
fn dropped_connection_reconnects_for_download() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    console.drop_connections();

    client.download_asset(&game(), AssetKind::Cover).unwrap();

    assert_eq!(console.login_count(), 2);
}

#[test]
fn dropped_connection_reconnects_for_upload() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    console.drop_connections();

    client
        .upload_asset(&game(), AssetKind::Cover, &empty_asset_bytes())
        .unwrap();

    assert_eq!(console.login_count(), 2);
    assert_eq!(console.file(COVER_FILE), Some(empty_asset_bytes()));
}

#[test]
fn dropped_connection_reconnects_for_presence() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    let mut client = client_for(&console);
    client.ensure_connected().unwrap();
    console.drop_connections();

    client.asset_presence(&game()).unwrap();

    assert_eq!(console.login_count(), 2);
}

#[test]
fn two_downloads_share_one_connection() {
    let console = console_with_game_folder(FakeConsoleConfig::default());
    console.add_file(COVER_FILE, &cover_asset_bytes(200));
    let mut client = client_for(&console);

    client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap()
        .unwrap();
    client
        .download_asset(&game(), AssetKind::Cover)
        .unwrap()
        .unwrap();

    assert_eq!(commands_containing(&console, "RETR").len(), 2);
    assert_eq!(console.login_count(), 1);
}
