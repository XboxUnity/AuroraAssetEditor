//! `log::set_logger` succeeds once per process, so this binary holds the only test that installs the bridge.

use aurora_asset_editor::logging::{ErrorLog, LogBridge};
use log::LevelFilter;
use tempfile::TempDir;

#[test]
fn install_sets_warn_as_max_level() {
    let dir = TempDir::new().expect("a temp dir is needed for the log file");
    let log = ErrorLog::in_dir(dir.path());

    LogBridge::install(log.clone()).expect("no other logger is installed in this binary");

    assert_eq!(log::max_level(), LevelFilter::Warn);
    log::info!(target: "aurora_online", "not recorded");
    log::trace!(target: "suppaftp", "CC OUT: PASS xboxftp");
    log::warn!(target: "aurora_online", "recorded");
    let text = std::fs::read_to_string(log.path()).expect("the warning creates the log");
    assert!(text.contains("] aurora_online: recorded\n\n"));
    assert!(!text.contains("not recorded"));
    assert!(!text.contains("PASS"));
}
