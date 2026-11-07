//! `log::set_logger` succeeds once per process, so this binary holds the only test that installs the bridge.

use aurora_asset_editor::logging::{ErrorLog, LogBridge};
use aurora_asset_editor::settings::{APP_SETTINGS_FILE, AppSettings};
use tempfile::TempDir;

#[test]
fn unknown_theme_name_is_logged_with_key_and_value() {
    let dir = TempDir::new().expect("a temp dir is needed for the files");
    let log = ErrorLog::in_dir(dir.path());
    LogBridge::install(log.clone()).expect("no other logger is installed in this binary");
    let path = dir.path().join(APP_SETTINGS_FILE);
    std::fs::write(&path, r#"{"theme": "sepia"}"#).unwrap();

    AppSettings::load(&path).unwrap();

    let text = std::fs::read_to_string(log.path()).expect("the warning creates the log");
    assert!(
        text.contains(r#"settings key "theme" has unknown value "sepia""#),
        "{text}"
    );
}
