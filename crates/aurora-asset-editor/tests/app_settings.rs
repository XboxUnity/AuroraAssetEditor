use aurora_asset_editor::settings::{
    APP_SETTINGS_FILE, AppSettings, AppSettingsError, MIN_HEIGHT, MIN_WIDTH, MonitorRect,
    ThemeChoice, WindowGeometry,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn settings_path(dir: &TempDir) -> PathBuf {
    dir.path().join(APP_SETTINGS_FILE)
}

fn temp_dir() -> TempDir {
    TempDir::new().expect("a temp dir is needed for the settings file")
}

fn geometry_at(x: i32, y: i32) -> WindowGeometry {
    WindowGeometry {
        x,
        y,
        width: 1024,
        height: 680,
        maximized: false,
    }
}

const FIRST_MONITOR: MonitorRect = MonitorRect {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};
const SECOND_MONITOR: MonitorRect = MonitorRect {
    x: 1920,
    y: 0,
    width: 1920,
    height: 1080,
};

#[test]
fn settings_default_to_old_app_toggles() {
    let settings = AppSettings::default();

    assert!(settings.auto_resize);
    assert!(settings.use_compression);
    assert_eq!(settings.theme, ThemeChoice::System);
    assert_eq!(settings.window, None);
}

#[test]
fn settings_round_trip() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    let settings = AppSettings {
        auto_resize: false,
        use_compression: false,
        theme: ThemeChoice::Dark,
        window: Some(WindowGeometry {
            x: -1900,
            y: 40,
            width: 900,
            height: 600,
            maximized: true,
        }),
    };

    settings.save(&path).unwrap();

    assert_eq!(AppSettings::load(&path).unwrap(), Some(settings));
}

#[test]
fn saved_json_uses_the_documented_keys() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    AppSettings {
        theme: ThemeChoice::Light,
        ..AppSettings::default()
    }
    .save(&path)
    .unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    assert_eq!(json["auto_resize"], true);
    assert_eq!(json["use_compression"], true);
    assert_eq!(json["theme"], "light");
    assert!(json["window"].is_null());
}

#[test]
fn missing_settings_file_is_none() {
    let dir = temp_dir();

    assert_eq!(AppSettings::load(&settings_path(&dir)).unwrap(), None);
}

#[test]
fn malformed_settings_fall_back_to_defaults_and_report() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    std::fs::write(&path, "{ not json").unwrap();

    let error = AppSettings::load(&path).unwrap_err();

    assert!(matches!(error, AppSettingsError::Malformed { .. }));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
}

#[test]
fn wrong_typed_known_key_is_malformed() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    std::fs::write(&path, r#"{"auto_resize": "yes"}"#).unwrap();

    let error = AppSettings::load(&path).unwrap_err();

    assert!(matches!(error, AppSettingsError::Malformed { .. }));
}

#[test]
fn unknown_theme_name_is_malformed() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    std::fs::write(&path, r#"{"theme": "sepia"}"#).unwrap();

    assert!(matches!(
        AppSettings::load(&path),
        Err(AppSettingsError::Malformed { .. })
    ));
}

#[test]
fn unknown_settings_keys_are_ignored() {
    let dir = temp_dir();
    let path = settings_path(&dir);
    std::fs::write(
        &path,
        r#"{"auto_resize": false, "font_scale": 1.5, "recent_files": ["a.asset"]}"#,
    )
    .unwrap();

    let settings = AppSettings::load(&path).unwrap().unwrap();

    assert_eq!(
        settings,
        AppSettings {
            auto_resize: false,
            ..AppSettings::default()
        }
    );
}

#[test]
fn settings_save_leaves_no_temp_file() {
    let dir = temp_dir();
    let path = settings_path(&dir);

    AppSettings::default().save(&path).unwrap();
    AppSettings::default().save(&path).unwrap();

    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, [APP_SETTINGS_FILE]);
}

#[test]
fn settings_save_creates_a_missing_folder() {
    let dir = temp_dir();
    let path = dir.path().join("config").join(APP_SETTINGS_FILE);

    AppSettings::default().save(&path).unwrap();

    assert!(path.exists());
}

#[test]
fn theme_choice_resolves_against_system() {
    assert!(ThemeChoice::System.is_dark(true));
    assert!(!ThemeChoice::System.is_dark(false));
    assert!(!ThemeChoice::Light.is_dark(true));
    assert!(ThemeChoice::Dark.is_dark(false));
}

#[test]
fn saved_position_off_every_monitor_is_ignored() {
    let monitors = [FIRST_MONITOR];

    assert_eq!(geometry_at(5000, 100).visible_position(&monitors), None);
    assert_eq!(geometry_at(100, 5000).visible_position(&monitors), None);
    assert_eq!(geometry_at(-3000, 100).visible_position(&monitors), None);
    assert_eq!(geometry_at(100, 100).visible_position(&[]), None);
}

#[test]
fn position_with_too_little_of_the_title_bar_on_screen_is_ignored() {
    let monitors = [FIRST_MONITOR];

    assert_eq!(
        geometry_at(1920 - 63, 100).visible_position(&monitors),
        None
    );
    assert_eq!(
        geometry_at(1920 - 64, 100).visible_position(&monitors),
        Some((1920 - 64, 100))
    );
    assert_eq!(
        geometry_at(100, 1080 - 31).visible_position(&monitors),
        None
    );
    assert_eq!(
        geometry_at(100, 1080 - 32).visible_position(&monitors),
        Some((100, 1080 - 32))
    );
}

#[test]
fn position_on_second_monitor_is_kept() {
    let monitors = [FIRST_MONITOR, SECOND_MONITOR];

    assert_eq!(
        geometry_at(2000, 50).visible_position(&monitors),
        Some((2000, 50))
    );
}

#[test]
fn too_small_saved_size_uses_minimum() {
    let tiny = WindowGeometry {
        width: 100,
        height: 50,
        ..geometry_at(0, 0)
    };

    assert_eq!(tiny.clamped_size(), (MIN_WIDTH, MIN_HEIGHT));
    assert_eq!(geometry_at(0, 0).clamped_size(), (1024, 680));
}
