#![forbid(unsafe_code)]

mod ids;
mod paths;
mod settings;

pub use ids::{GameIds, HexId, IdError};
pub use paths::{CONTENT_DB_FILE, DATABASES_DIR, GAME_DATA_DIR, asset_file_name, asset_folder};
pub use settings::{
    FTP_SETTINGS_FILE, FtpSettings, SettingsError, app_config_dir, local_subnet_prefix,
};
