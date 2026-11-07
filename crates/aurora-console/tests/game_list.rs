use std::path::{Path, PathBuf};
use std::time::Duration;

use aurora_console::{ConsoleClient, ConsoleError, FtpSettings, Timeouts, read_content_db};
use test_servers::{FakeConsole, FakeConsoleConfig};

const SHORT: Duration = Duration::from_millis(500);
const NOT_SQLITE_LEN: usize = 4096;

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/console/content-db")
        .join(name)
}

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

fn is_empty_dir(path: &Path) -> bool {
    std::fs::read_dir(path)
        .expect("the temp dir is readable")
        .next()
        .is_none()
}

#[test]
fn lists_games_from_console_content_db() {
    let typical = std::fs::read(sample("typical.db")).expect("the sample database is readable");
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &typical);
    let temp_dir = tempfile::tempdir().expect("a temp dir is created");

    let listing = client_for(&console)
        .list_games_in(temp_dir.path())
        .expect("the games are listed");

    let expected = read_content_db(&sample("typical.db")).expect("the sample database reads");
    assert_eq!(listing.games, expected);
    let commands = console.commands();
    for wanted in [
        "CWD /Game/Data/DataBases",
        "LIST /Game/Data/DataBases",
        "RETR Content.db",
    ] {
        assert!(
            commands.iter().any(|command| command == wanted),
            "{wanted} was not sent: {commands:?}"
        );
    }
    assert!(listing.cleanup_error.is_none());
    assert!(is_empty_dir(temp_dir.path()));
}

#[test]
fn missing_content_db_is_reported() {
    let console = FakeConsole::start(FakeConsoleConfig::default());
    console.add_dir("/Game/Data/DataBases");
    let temp_dir = tempfile::tempdir().expect("a temp dir is created");

    let error = client_for(&console)
        .list_games_in(temp_dir.path())
        .unwrap_err();

    assert!(matches!(error, ConsoleError::ContentDbMissing), "{error:?}");
    assert!(is_empty_dir(temp_dir.path()));
}

#[test]
fn empty_content_db_is_reported() {
    let console = FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &[]);
    let temp_dir = tempfile::tempdir().expect("a temp dir is created");

    let error = client_for(&console)
        .list_games_in(temp_dir.path())
        .unwrap_err();

    assert!(matches!(error, ConsoleError::ContentDbMissing), "{error:?}");
    assert!(is_empty_dir(temp_dir.path()));
}

#[test]
fn unreadable_content_db_is_error_and_cleaned_up() {
    let console =
        FakeConsole::with_aurora_layout(FakeConsoleConfig::default(), &[7; NOT_SQLITE_LEN]);
    let temp_dir = tempfile::tempdir().expect("a temp dir is created");

    let error = client_for(&console)
        .list_games_in(temp_dir.path())
        .unwrap_err();

    assert!(matches!(error, ConsoleError::ContentDb(_)), "{error:?}");
    assert!(is_empty_dir(temp_dir.path()));
}
