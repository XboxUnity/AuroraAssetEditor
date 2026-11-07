#[allow(
    clippy::module_inception,
    reason = "each file is named after the type it holds"
)]
mod app;
mod app_environment;
mod current_game;
mod current_game_actions;
mod file_actions;
mod kind_choice;
mod page;
mod settings_actions;
mod startup_settings;

pub use app::{App, DIALOG_FAILED_TITLE, LOG_WRITE_FAILED};
pub use app_environment::{AppEnvironment, StartupProblem};
pub use current_game::CurrentGame;
pub use kind_choice::KindChoice;
pub use page::Page;
pub use settings_actions::SETTINGS_SAVE_FAILED;
pub use startup_settings::{SETTINGS_LOAD_FAILED, StartupSettings};
