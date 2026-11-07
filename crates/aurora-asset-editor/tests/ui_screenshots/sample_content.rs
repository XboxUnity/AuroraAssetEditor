use aurora_asset_editor::app::CurrentGame;
use aurora_console::{FtpSettings, GameIds, HexId};
use editor_test_support::AppHarness;

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
