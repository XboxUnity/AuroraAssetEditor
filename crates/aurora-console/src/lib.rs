#![forbid(unsafe_code)]

#[expect(unused_imports, reason = "the FTP client will be the first user")]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the FTP client will be the first user")
)]
mod ftp;
mod games;
mod ids;
mod paths;
mod settings;

pub use games::{ContentDbError, Game, GameFilter, read_content_db};
pub use ids::{GameIds, HexId, IdError};
pub use paths::{CONTENT_DB_FILE, DATABASES_DIR, GAME_DATA_DIR, asset_file_name, asset_folder};
pub use settings::{
    FTP_SETTINGS_FILE, FtpSettings, SettingsError, app_config_dir, local_subnet_prefix,
};
