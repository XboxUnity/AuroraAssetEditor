use aurora_console::{FtpSettings, SettingsError};
use std::fs;
use std::path::Path;

const OLD_APP_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/console/settings/old-app-ftp.json"
);

fn settings(host: &str, port: &str, username: &str, password: &str) -> FtpSettings {
    FtpSettings {
        host: host.to_string(),
        port: port.to_string(),
        username: username.to_string(),
        password: password.to_string(),
    }
}

fn load_text(text: &str) -> Result<Option<FtpSettings>, SettingsError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ftp.json");
    fs::write(&path, text).unwrap();
    FtpSettings::load(&path)
}

#[test]
fn reads_old_app_ftp_json() {
    let loaded = FtpSettings::load(Path::new(OLD_APP_FIXTURE)).unwrap();
    assert_eq!(
        loaded,
        Some(settings("192.168.1.50", "21", "xboxftp", "p/a\"ss\\é"))
    );
}

#[test]
fn missing_keys_and_nulls_read_as_empty() {
    let loaded = load_text(r#"{"ip":null,"port":"2121"}"#).unwrap();
    assert_eq!(loaded, Some(settings("", "2121", "", "")));
}

#[test]
fn missing_file_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = FtpSettings::load(&dir.path().join("absent.json")).unwrap();
    assert_eq!(loaded, None);
}

#[test]
fn malformed_json_is_error() {
    let result = load_text(r#"{"ip":"#);
    assert!(matches!(result, Err(SettingsError::Malformed { .. })));
}

#[test]
fn unreadable_path_is_read_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = FtpSettings::load(dir.path());
    assert!(matches!(result, Err(SettingsError::Read { .. })));
}

#[test]
fn save_writes_old_app_compatible_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-yet").join("ftp.json");
    let saved = settings("192.168.1.50", "21", "xboxftp", "p/a\"ss\\é");

    saved.save(&path).unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "no BOM");
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        r#"{"ip":"192.168.1.50","pass":"p/a\"ss\\é","port":"21","user":"xboxftp"}"#
    );
    assert_eq!(FtpSettings::load(&path).unwrap(), Some(saved));
}

#[test]
fn port_number_rules() {
    let port = |text: &str| settings("", text, "", "").port_number();

    assert_eq!(port("21").unwrap(), 21);
    assert_eq!(port(" 2121 ").unwrap(), 2121);
    assert_eq!(port("+21").unwrap(), 21);
    assert_eq!(port("\t21\r\n").unwrap(), 21);
    assert_eq!(port("2121\0").unwrap(), 2121);
    for fallback in [
        "",
        "abc",
        "3000000000",
        "2147483648",
        "+",
        "- 5",
        "--5",
        "\u{a0}2121",
        "0",
    ] {
        assert_eq!(port(fallback).unwrap(), 21, "{fallback:?}");
    }
    for (text, value) in [
        ("70000", 70000),
        ("-1", -1),
        ("2147483647", i32::MAX),
        ("-2147483648", i32::MIN),
    ] {
        assert!(
            matches!(port(text), Err(SettingsError::PortOutOfRange { port }) if port == value),
            "{text}"
        );
    }
}

#[test]
fn load_accepts_utf8_bom() {
    let loaded = load_text("\u{feff}{\"ip\":\"10.0.0.1\"}").unwrap();
    assert_eq!(loaded, Some(settings("10.0.0.1", "", "", "")));
}

#[test]
fn defaults_match_old_dialog() {
    assert_eq!(
        FtpSettings::default(),
        settings("", "21", "xboxftp", "xboxftp")
    );
}

#[cfg(windows)]
#[test]
fn app_config_dir_matches_old_location() {
    let appdata = std::env::var("APPDATA").unwrap();
    assert_eq!(
        aurora_console::app_config_dir(),
        Some(std::path::PathBuf::from(appdata).join("AuroraAssetEditor"))
    );
}
