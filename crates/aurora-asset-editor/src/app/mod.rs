#[allow(
    clippy::module_inception,
    reason = "each file is named after the type it holds"
)]
mod app;
mod app_environment;
mod bulk_actions;
mod console_actions;
mod console_transfer_actions;
mod current_game;
mod current_game_actions;
mod file_actions;
mod image_actions;
mod kind_choice;
mod online_actions;
mod page;
mod pending_import_status;
mod save_actions;
mod settings_actions;
mod startup_settings;

pub use app::{App, DIALOG_FAILED_TITLE, LOG_WRITE_FAILED};
pub use app_environment::{AppEnvironment, StartupProblem};
pub use bulk_actions::{BULK_FINISHED_TITLE, BULK_TITLE, NO_ASSETS_TEXT};
pub use console_actions::{
    CONNECTION_FAILED_TITLE, NO_CONSOLE_TEXT, REFRESHING_TEXT, TEMP_FILE_NOTE,
};
pub use console_transfer_actions::{
    EMPTY_UPLOAD_TITLE, NO_CONSOLE_SET_UP_REASON, NOTHING_TO_UPLOAD_TEXT, REMOVE_CONFIRM_TITLE,
    SELECT_A_GAME_REASON, UPLOAD_TARGET_TITLE,
};
pub use current_game::CurrentGame;
pub use file_actions::BUSY_TEXT;
pub use kind_choice::KindChoice;
pub use online_actions::{
    ENTER_KEYWORDS_TEXT, ENTER_TITLE_ID_TEXT, IMAGE_FAILED_TEXT, IMPORTING_COVER_TEXT,
    SAVE_RESULT_DEFAULT_NAME, SAVE_RESULT_TITLE, SEARCH_FAILED,
};
pub use page::Page;
pub use pending_import_status::PendingImportStatus;
pub use settings_actions::SETTINGS_SAVE_FAILED;
pub use startup_settings::{SETTINGS_LOAD_FAILED, StartupSettings};
