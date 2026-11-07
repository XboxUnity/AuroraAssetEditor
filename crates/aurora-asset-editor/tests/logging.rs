use aurora_asset_editor::logging::{
    ErrorLog, LogBridge, SUPPAFTP_MAX_LEVEL, SUPPAFTP_TARGET, error_chain,
};
use log::{Level, Log, Metadata, Record};
use std::error::Error;
use std::fmt;
use tempfile::TempDir;
use test_servers::{FAKE_INPUT_HASH, FAKE_STORED_HASH};

#[derive(Debug)]
struct Layered {
    message: &'static str,
    source: Option<Box<Layered>>,
}

impl fmt::Display for Layered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Error for Layered {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn two_level_error() -> Layered {
    Layered {
        message: "outer",
        source: Some(Box::new(Layered {
            message: "inner",
            source: None,
        })),
    }
}

fn log_in_temp_dir() -> (TempDir, ErrorLog) {
    let dir = TempDir::new().expect("a temp dir is needed for the log file");
    let log = ErrorLog::in_dir(dir.path());
    (dir, log)
}

fn read_log(log: &ErrorLog) -> String {
    std::fs::read_to_string(log.path()).unwrap_or_default()
}

fn send(bridge: &LogBridge, level: Level, target: &str, message: &str) {
    bridge.log(
        &Record::builder()
            .args(format_args!("{message}"))
            .level(level)
            .target(target)
            .build(),
    );
}

/// `9` stands for a digit, `+` for a sign; every other character must match exactly.
fn matches_shape(text: &str, shape: &str) -> bool {
    text.chars().count() == shape.chars().count()
        && text
            .chars()
            .zip(shape.chars())
            .all(|(actual, expected)| match expected {
                '9' => actual.is_ascii_digit(),
                '+' => actual == '+' || actual == '-',
                other => actual == other,
            })
}

const ENTRY_PREFIX_SHAPE: &str = "[9999-99-99 99:99:99 +99:99] ";

#[test]
fn errors_are_logged_with_timestamp_and_chain() {
    let (_dir, log) = log_in_temp_dir();

    log.record("ctx", &two_level_error()).unwrap();

    let text = read_log(&log);
    let (prefix, rest) = text.split_at(ENTRY_PREFIX_SHAPE.len());
    assert!(
        matches_shape(prefix, ENTRY_PREFIX_SHAPE),
        "prefix was {prefix:?}"
    );
    assert_eq!(rest, "ctx: outer\n  caused by: inner\n\n");
}

#[test]
fn error_chain_lists_the_error_and_each_source() {
    assert_eq!(error_chain(&two_level_error()), ["outer", "inner"]);
}

#[test]
fn log_appends_entries() {
    let (_dir, log) = log_in_temp_dir();

    log.record_message("first", "one").unwrap();
    log.record("second", &two_level_error()).unwrap();

    let text = read_log(&log);
    assert_eq!(text.matches("] first: one\n\n").count(), 1);
    assert_eq!(
        text.matches("] second: outer\n  caused by: inner\n\n")
            .count(),
        1
    );
    assert!(text.find("first").unwrap() < text.find("second").unwrap());
}

#[test]
fn log_creates_a_missing_config_folder() {
    let dir = TempDir::new().unwrap();
    let log = ErrorLog::in_dir(&dir.path().join("not").join("yet"));

    log.record_message("ctx", "message").unwrap();

    assert!(read_log(&log).contains("] ctx: message\n\n"));
}

#[test]
fn log_bridge_writes_warnings_and_errors() {
    let (_dir, log) = log_in_temp_dir();
    let bridge = LogBridge::new(log.clone());

    send(&bridge, Level::Warn, "aurora_online", "slow reply");
    send(&bridge, Level::Error, "aurora_console", "connection lost");

    let text = read_log(&log);
    assert!(text.contains("] aurora_online: slow reply\n\n"));
    assert!(text.contains("] aurora_console: connection lost\n\n"));
}

#[test]
fn log_bridge_ignores_info() {
    let (_dir, log) = log_in_temp_dir();
    let bridge = LogBridge::new(log.clone());

    send(&bridge, Level::Info, "aurora_online", "request sent");
    send(&bridge, Level::Debug, "aurora_online", "details");
    send(&bridge, Level::Trace, "aurora_online", "bytes");

    assert!(!log.path().exists());
}

#[test]
fn log_bridge_never_writes_suppaftp_control_channel() {
    let (_dir, log) = log_in_temp_dir();
    let bridge = LogBridge::new(log.clone());
    let reply = format!(
        "CC IN: \"530 Login failed: input = {FAKE_INPUT_HASH}, pass = {FAKE_STORED_HASH}\""
    );
    let pass_command = "CC OUT: PASS xboxftp";

    for target in [SUPPAFTP_TARGET, "suppaftp::sync_ftp"] {
        for level in [Level::Trace, Level::Debug, Level::Info] {
            send(&bridge, level, target, &reply);
            send(&bridge, level, target, pass_command);
            let metadata = Metadata::builder().level(level).target(target).build();
            assert!(
                !bridge.enabled(&metadata),
                "{target} at {level} must be disabled"
            );
        }
    }

    let text = read_log(&log);
    assert!(!text.contains(FAKE_STORED_HASH));
    assert!(!text.contains(FAKE_INPUT_HASH));
    assert!(!text.contains("PASS"));
    assert!(text.is_empty());
}

#[test]
fn suppaftp_warnings_stay_visible() {
    let (_dir, log) = log_in_temp_dir();
    let bridge = LogBridge::new(log.clone());

    send(
        &bridge,
        SUPPAFTP_MAX_LEVEL.to_level().unwrap(),
        SUPPAFTP_TARGET,
        "transfer stream failed",
    );

    assert!(read_log(&log).contains("] suppaftp: transfer stream failed\n\n"));
}

#[test]
fn suppaftp_cap_holds_when_the_bridge_level_is_raised() {
    let (_dir, log) = log_in_temp_dir();
    let bridge = LogBridge::with_max_level(log.clone(), log::LevelFilter::Trace);

    send(
        &bridge,
        Level::Trace,
        SUPPAFTP_TARGET,
        "CC OUT: PASS xboxftp",
    );
    send(&bridge, Level::Debug, "suppaftp::sync_ftp", "CC IN: secret");
    send(
        &bridge,
        Level::Trace,
        "aurora_online",
        "other crates still pass",
    );

    let text = read_log(&log);
    assert!(!text.contains("PASS"));
    assert!(!text.contains("secret"));
    assert!(text.contains(
        "] aurora_online: other crates still pass

"
    ));
}
