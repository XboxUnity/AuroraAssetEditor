#![forbid(unsafe_code)]

mod ids;
mod paths;

pub use ids::{GameIds, HexId, IdError};
pub use paths::{CONTENT_DB_FILE, DATABASES_DIR, GAME_DATA_DIR, asset_file_name, asset_folder};
