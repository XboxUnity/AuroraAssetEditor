#![forbid(unsafe_code)]

mod bulk;
mod client;
mod ftp;
mod games;
mod ids;
mod paths;
mod settings;

pub use bulk::{
    BulkEntry, BulkError, BulkFailure, BulkOptions, BulkOutcome, BulkProgress, BulkReport,
    BulkStop, BulkSummary, CoverProvider, run_bulk_covers,
};
pub use client::{
    AssetPresence, AuroraRevision, ConsoleClient, ConsoleError, ConsoleStatus, GameListing,
    TempCleanupError, Timeouts,
};
pub use games::{ContentDbError, Game, GameFilter, read_content_db};
pub use ids::{GameIds, HexId, IdError};
pub use paths::{CONTENT_DB_FILE, DATABASES_DIR, GAME_DATA_DIR, asset_file_name, asset_folder};
pub use settings::{
    FTP_SETTINGS_FILE, FtpSettings, SettingsError, app_config_dir, local_subnet_prefix,
};
