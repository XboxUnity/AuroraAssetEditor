use std::collections::HashMap;
use std::fmt;
use std::ops::ControlFlow;
use std::time::Duration;

use aurora_asset::{
    AssetFile, AssetSlot, Compression, EncodedTexture, RgbaImage, TEXTURE_HEADER_LEN,
    TextureFormat, TextureHeader, encode,
};
use aurora_console::{
    BulkEntry, BulkError, BulkFailure, BulkOptions, BulkOutcome, BulkReport, BulkStop, BulkSummary,
    ConsoleClient, ConsoleError, CoverProvider, FtpSettings, Game, GameIds, HexId, Timeouts,
    run_bulk_covers,
};
use test_servers::{FakeConsole, FakeConsoleConfig};

const HOST: &str = "127.0.0.1";
const SHORT: Duration = Duration::from_millis(500);
const EMPTY_ASSET_LEN: usize = 2048;
const DATABASE_ID: &str = "00000012";
const FIRST_TITLE: &str = "4D5307E6";
const SECOND_TITLE: &str = "4D5307E7";
const THIRD_TITLE: &str = "4D5307E8";
const COVER_WIDTH: u32 = 37;
const COVER_HEIGHT: u32 = 23;

#[derive(Debug)]
struct FakeError(String);

impl fmt::Display for FakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FakeError {}

struct FakeProvider<'a> {
    answers: HashMap<String, Result<Option<RgbaImage>, String>>,
    calls: Vec<String>,
    on_call: Option<Box<dyn FnMut() + 'a>>,
}

impl<'a> FakeProvider<'a> {
    fn new() -> Self {
        FakeProvider {
            answers: HashMap::new(),
            calls: Vec::new(),
            on_call: None,
        }
    }

    fn with_cover(mut self, title_id: &str, image: RgbaImage) -> Self {
        self.answers.insert(title_id.to_owned(), Ok(Some(image)));
        self
    }

    fn without_cover(mut self, title_id: &str) -> Self {
        self.answers.insert(title_id.to_owned(), Ok(None));
        self
    }

    fn failing(mut self, title_id: &str, message: &str) -> Self {
        self.answers
            .insert(title_id.to_owned(), Err(message.to_owned()));
        self
    }

    fn on_call(mut self, hook: impl FnMut() + 'a) -> Self {
        self.on_call = Some(Box::new(hook));
        self
    }
}

impl CoverProvider for FakeProvider<'_> {
    type Error = FakeError;

    fn random_cover(&mut self, title_id: &HexId) -> Result<Option<RgbaImage>, FakeError> {
        self.calls.push(title_id.as_str().to_owned());
        if let Some(hook) = self.on_call.as_mut() {
            hook();
        }
        match self.answers.get(title_id.as_str()) {
            Some(Ok(image)) => Ok(image.clone()),
            Some(Err(message)) => Err(FakeError(message.clone())),
            None => panic!("the provider was asked for an unexpected title {title_id:?}"),
        }
    }
}

fn client_for(console: &FakeConsole) -> ConsoleClient {
    client_with_password(console, "xboxftp")
}

fn client_with_password(console: &FakeConsole, password: &str) -> ConsoleClient {
    ConsoleClient::new(
        FtpSettings {
            host: HOST.to_owned(),
            port: console.port().to_string(),
            username: "xboxftp".to_owned(),
            password: password.to_owned(),
        },
        Timeouts {
            connect: SHORT,
            read: SHORT,
        },
        |_| {},
    )
}

fn hex(text: &str) -> HexId {
    HexId::parse(text).unwrap()
}

fn game(title_id: &str) -> Game {
    Game {
        title_name: format!("Game {title_id}"),
        ids: GameIds {
            title_id: hex(title_id),
            database_id: hex(DATABASE_ID),
        },
        media_id: hex("00000000"),
        disc: 1,
    }
}

fn folder(title_id: &str) -> String {
    format!("/Game/Data/GameData/{title_id}_{DATABASE_ID}")
}

fn cover_path(title_id: &str) -> String {
    format!("{}/GC{title_id}.asset", folder(title_id))
}

fn console_with_games(title_ids: &[&str]) -> FakeConsole {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    for title_id in title_ids {
        console.add_dir(&folder(title_id));
    }
    console
}

fn cover_image() -> RgbaImage {
    RgbaImage::from_pixel(COVER_WIDTH, COVER_HEIGHT, image::Rgba([200, 40, 90, 255]))
}

fn options(compression: Compression) -> BulkOptions {
    BulkOptions {
        replace_existing: false,
        compression,
    }
}

fn asset_bytes_with_cover(texture: EncodedTexture) -> Vec<u8> {
    let mut file = AssetFile::new();
    file.set(AssetSlot::Cover, Some(texture));
    file.to_bytes()
}

fn existing_cover_bytes() -> Vec<u8> {
    let small = RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]));
    asset_bytes_with_cover(encode(&small, Compression::None).unwrap())
}

fn run(
    console: &FakeConsole,
    titles: &[&str],
    provider: &mut FakeProvider<'_>,
    options: BulkOptions,
) -> BulkReport {
    let games: Vec<Game> = titles.iter().map(|title| game(title)).collect();
    run_bulk_covers(&mut client_for(console), &games, provider, options, |_| {
        ControlFlow::Continue(())
    })
    .unwrap()
}

fn retrievals(console: &FakeConsole) -> Vec<String> {
    console
        .commands()
        .into_iter()
        .filter(|command| command.starts_with("RETR "))
        .collect()
}

fn only_outcome(report: &BulkReport) -> &BulkOutcome {
    assert_eq!(report.entries.len(), 1, "{report:?}");
    &report.entries[0].outcome
}

#[test]
fn bulk_uploads_cover_when_missing() {
    let console = console_with_games(&[FIRST_TITLE]);
    let mut provider = FakeProvider::new().with_cover(FIRST_TITLE, cover_image());

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(only_outcome(&report), BulkOutcome::Uploaded));
    assert!(report.stopped.is_none());
    let uploaded = console.file(&cover_path(FIRST_TITLE)).unwrap();
    let parsed = AssetFile::parse(&uploaded).unwrap();
    let cover = parsed.get(AssetSlot::Cover).unwrap();
    assert_eq!(cover.header.width(), COVER_WIDTH);
    assert_eq!(cover.header.height(), COVER_HEIGHT);
    let only_cover = AssetFile::parse(&asset_bytes_with_cover(cover.clone())).unwrap();
    assert_eq!(parsed, only_cover, "the file holds only the cover");
    let expected = asset_bytes_with_cover(encode(&cover_image(), Compression::Dxt).unwrap());
    assert_eq!(uploaded, expected);
}

#[test]
fn bulk_uses_compression_setting() {
    let console = console_with_games(&[FIRST_TITLE]);
    let mut provider = FakeProvider::new().with_cover(FIRST_TITLE, cover_image());

    run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::None),
    );

    let uploaded = console.file(&cover_path(FIRST_TITLE)).unwrap();
    let parsed = AssetFile::parse(&uploaded).unwrap();
    let cover = parsed.get(AssetSlot::Cover).unwrap();
    assert_eq!(cover.header.format(), TextureFormat::LinearArgb8888);
    let expected = asset_bytes_with_cover(encode(&cover_image(), Compression::None).unwrap());
    assert_eq!(uploaded, expected);
}

#[test]
fn bulk_keeps_existing_cover() {
    let console = console_with_games(&[FIRST_TITLE]);
    let existing = existing_cover_bytes();
    console.add_file(&cover_path(FIRST_TITLE), &existing);
    let mut provider = FakeProvider::new();

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(
        only_outcome(&report),
        BulkOutcome::AlreadyHasCover
    ));
    assert!(provider.calls.is_empty());
    assert_eq!(console.file(&cover_path(FIRST_TITLE)).unwrap(), existing);
}

#[test]
fn bulk_treats_empty_asset_as_missing() {
    let console = console_with_games(&[FIRST_TITLE]);
    let empty = AssetFile::new().to_bytes();
    assert_eq!(empty.len(), EMPTY_ASSET_LEN);
    console.add_file(&cover_path(FIRST_TITLE), &empty);
    let mut provider = FakeProvider::new().with_cover(FIRST_TITLE, cover_image());

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(only_outcome(&report), BulkOutcome::Uploaded));
    let uploaded = AssetFile::parse(&console.file(&cover_path(FIRST_TITLE)).unwrap()).unwrap();
    assert!(uploaded.get(AssetSlot::Cover).is_some());
}

#[test]
fn bulk_skips_unreadable_cover() {
    let console = console_with_games(&[FIRST_TITLE]);
    let unreadable = vec![0xAB; EMPTY_ASSET_LEN];
    console.add_file(&cover_path(FIRST_TITLE), &unreadable);
    let mut provider = FakeProvider::new();

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(
        only_outcome(&report),
        BulkOutcome::ExistingCoverUnreadable(ConsoleError::InvalidAsset { .. })
    ));
    assert!(provider.calls.is_empty());
    assert_eq!(console.file(&cover_path(FIRST_TITLE)).unwrap(), unreadable);
}

#[test]
fn bulk_keeps_cover_in_unsupported_format() {
    let console = console_with_games(&[FIRST_TITLE]);
    let header = TextureHeader::from_bytes([0xFF; TEXTURE_HEADER_LEN]);
    assert!(
        matches!(header.format(), TextureFormat::Unsupported { .. }),
        "the crafted header must be a format the port cannot decode"
    );
    let existing = asset_bytes_with_cover(EncodedTexture {
        header,
        data: vec![0; 64],
    });
    console.add_file(&cover_path(FIRST_TITLE), &existing);
    let mut provider = FakeProvider::new();

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(
        only_outcome(&report),
        BulkOutcome::AlreadyHasCover
    ));
    assert!(provider.calls.is_empty());
    assert_eq!(console.file(&cover_path(FIRST_TITLE)).unwrap(), existing);
}

#[test]
fn bulk_replace_existing_skips_the_check() {
    let console = console_with_games(&[FIRST_TITLE]);
    let existing = existing_cover_bytes();
    console.add_file(&cover_path(FIRST_TITLE), &existing);
    let mut provider = FakeProvider::new().with_cover(FIRST_TITLE, cover_image());

    let report = run(
        &console,
        &[FIRST_TITLE],
        &mut provider,
        BulkOptions {
            replace_existing: true,
            compression: Compression::Dxt,
        },
    );

    assert!(matches!(only_outcome(&report), BulkOutcome::Uploaded));
    assert!(retrievals(&console).is_empty());
    assert_ne!(console.file(&cover_path(FIRST_TITLE)).unwrap(), existing);
}

#[test]
fn bulk_continues_after_game_errors() {
    let console = console_with_games(&[SECOND_TITLE, THIRD_TITLE]);
    let mut provider = FakeProvider::new()
        .with_cover(FIRST_TITLE, cover_image())
        .failing(SECOND_TITLE, "the server is down")
        .with_cover(THIRD_TITLE, cover_image());

    let report = run(
        &console,
        &[FIRST_TITLE, SECOND_TITLE, THIRD_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(report.stopped.is_none());
    let outcomes: Vec<&BulkOutcome> = report.entries.iter().map(|entry| &entry.outcome).collect();
    assert!(matches!(
        outcomes[0],
        BulkOutcome::Failed(BulkFailure::Console(ConsoleError::MissingFolder { .. }))
    ));
    assert!(matches!(
        outcomes[1],
        BulkOutcome::Failed(BulkFailure::Provider(_))
    ));
    assert!(matches!(outcomes[2], BulkOutcome::Uploaded));
    assert_eq!(outcomes.len(), 3);
    assert!(console.file(&cover_path(THIRD_TITLE)).is_some());
}

#[test]
fn bulk_reports_titles_without_covers() {
    let console = console_with_games(&[FIRST_TITLE, SECOND_TITLE]);
    let mut provider = FakeProvider::new()
        .without_cover(FIRST_TITLE)
        .with_cover(SECOND_TITLE, cover_image());

    let report = run(
        &console,
        &[FIRST_TITLE, SECOND_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(
        report.entries[0].outcome,
        BulkOutcome::NoCoverFound
    ));
    assert!(console.file(&cover_path(FIRST_TITLE)).is_none());
    let summary = report.summary();
    assert_eq!(summary.no_cover_online, 1);
    assert_eq!(summary.uploaded, 1);
}

#[test]
fn bulk_checks_connection_first() {
    let console = console_with_games(&[FIRST_TITLE]);
    let mut provider = FakeProvider::new();

    let error = run_bulk_covers(
        &mut client_with_password(&console, "wrong"),
        &[game(FIRST_TITLE)],
        &mut provider,
        BulkOptions {
            replace_existing: true,
            compression: Compression::Dxt,
        },
        |_| ControlFlow::Continue(()),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        BulkError::Connection(ConsoleError::LoginRejected { .. })
    ));
    assert!(provider.calls.is_empty());
}

#[test]
fn bulk_rejects_empty_list() {
    let console = console_with_games(&[]);
    let mut provider = FakeProvider::new();

    let error = run_bulk_covers(
        &mut client_for(&console),
        &[],
        &mut provider,
        options(Compression::Dxt),
        |_| ControlFlow::Continue(()),
    )
    .unwrap_err();

    assert!(matches!(error, BulkError::NoGames));
    assert_eq!(console.login_count(), 0);
}

#[test]
fn bulk_stops_when_console_is_gone() {
    let console = console_with_games(&[FIRST_TITLE, SECOND_TITLE, THIRD_TITLE]);
    let mut provider = FakeProvider::new()
        .with_cover(FIRST_TITLE, cover_image())
        .on_call(|| {
            console.drop_connections();
            console.refuse_new_connections();
        });

    let report = run(
        &console,
        &[FIRST_TITLE, SECOND_TITLE, THIRD_TITLE],
        &mut provider,
        options(Compression::Dxt),
    );

    assert!(matches!(
        only_outcome(&report),
        BulkOutcome::Failed(BulkFailure::Console(_))
    ));
    assert!(matches!(report.stopped, Some(BulkStop::ConsoleGone(_))));
    assert_eq!(report.total, 3);
    assert_eq!(report.summary().not_handled, 2);
    assert_eq!(provider.calls, [FIRST_TITLE]);
}

#[test]
fn bulk_progress_counts_positions() {
    let console = console_with_games(&[FIRST_TITLE, SECOND_TITLE, THIRD_TITLE]);
    let mut provider = FakeProvider::new()
        .without_cover(FIRST_TITLE)
        .without_cover(SECOND_TITLE)
        .without_cover(THIRD_TITLE);
    let games = [game(FIRST_TITLE), game(SECOND_TITLE), game(THIRD_TITLE)];
    let mut seen = Vec::new();

    run_bulk_covers(
        &mut client_for(&console),
        &games,
        &mut provider,
        options(Compression::Dxt),
        |progress| {
            seen.push((
                progress.position,
                progress.total,
                progress.game.title_name.clone(),
            ));
            ControlFlow::Continue(())
        },
    )
    .unwrap();

    let positions: Vec<(usize, usize)> = seen.iter().map(|(p, t, _)| (*p, *t)).collect();
    assert_eq!(positions, [(1, 3), (2, 3), (3, 3)]);
    assert_eq!(seen[1].2, games[1].title_name);
}

#[test]
fn summary_reads_like_a_sentence() {
    let outcome = |outcome| BulkEntry {
        game: game(FIRST_TITLE),
        outcome,
    };
    let mut entries = Vec::new();
    for _ in 0..3 {
        entries.push(outcome(BulkOutcome::Uploaded));
    }
    for _ in 0..2 {
        entries.push(outcome(BulkOutcome::AlreadyHasCover));
    }
    entries.push(outcome(BulkOutcome::ExistingCoverUnreadable(
        ConsoleError::ContentDbMissing,
    )));
    entries.push(outcome(BulkOutcome::NoCoverFound));
    entries.push(outcome(BulkOutcome::Failed(BulkFailure::Console(
        ConsoleError::ContentDbMissing,
    ))));
    let report = BulkReport {
        total: entries.len(),
        entries,
        stopped: None,
    };

    let summary: BulkSummary = report.summary();

    assert_eq!(
        summary,
        BulkSummary {
            uploaded: 3,
            already_had_cover: 2,
            unreadable: 1,
            no_cover_online: 1,
            failed: 1,
            not_handled: 0,
        }
    );
}

#[test]
fn bulk_stops_before_the_next_game_when_cancelled() {
    let console = console_with_games(&[FIRST_TITLE, SECOND_TITLE, THIRD_TITLE]);
    let mut provider = FakeProvider::new().with_cover(FIRST_TITLE, cover_image());
    let games = [game(FIRST_TITLE), game(SECOND_TITLE), game(THIRD_TITLE)];

    let report = run_bulk_covers(
        &mut client_for(&console),
        &games,
        &mut provider,
        options(Compression::Dxt),
        |progress| {
            if progress.position == 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .unwrap();

    assert!(matches!(only_outcome(&report), BulkOutcome::Uploaded));
    assert!(matches!(report.stopped, Some(BulkStop::Cancelled)));
    assert_eq!(report.summary().not_handled, 2);
    assert_eq!(provider.calls, [FIRST_TITLE]);
    assert!(console.file(&cover_path(FIRST_TITLE)).is_some());
    assert!(console.file(&cover_path(SECOND_TITLE)).is_none());
    assert_eq!(
        report.summary(),
        BulkSummary {
            uploaded: 1,
            not_handled: 2,
            ..BulkSummary::default()
        }
    );
}

#[test]
fn bulk_cancelled_at_the_first_game_handles_nothing() {
    let console = console_with_games(&[FIRST_TITLE, SECOND_TITLE]);
    let mut provider = FakeProvider::new();
    let games = [game(FIRST_TITLE), game(SECOND_TITLE)];

    let report = run_bulk_covers(
        &mut client_for(&console),
        &games,
        &mut provider,
        options(Compression::Dxt),
        |_| ControlFlow::Break(()),
    )
    .unwrap();

    assert!(report.entries.is_empty());
    assert!(matches!(report.stopped, Some(BulkStop::Cancelled)));
    assert_eq!(report.summary().not_handled, 2);
    assert!(provider.calls.is_empty());
}
