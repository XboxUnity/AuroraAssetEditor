use crate::status::StatusLine;

use super::{App, CurrentGame};

const TITLE_ID_LABEL: &str = "TitleID";
const DATABASE_ID_LABEL: &str = "DatabaseID";

impl App {
    pub fn current_game(&self) -> Option<&CurrentGame> {
        self.current_game.as_ref()
    }

    pub fn set_current_game(&mut self, game: Option<CurrentGame>) {
        self.current_game = game;
        if self.current_game.is_none() {
            self.console.table.clear_selection();
        }
    }

    pub fn clear_current_game(&mut self) {
        self.set_current_game(None);
    }

    /// The game chip menu's "Copy TitleID {id}".
    pub fn copy_title_id(&mut self) {
        if let Some(game) = &self.current_game {
            let id = game.ids.title_id.to_string();
            self.copy_id(TITLE_ID_LABEL, &id);
        }
    }

    /// The game chip menu's "Copy DatabaseID {id}".
    pub fn copy_database_id(&mut self) {
        if let Some(game) = &self.current_game {
            let id = game.ids.database_id.to_string();
            self.copy_id(DATABASE_ID_LABEL, &id);
        }
    }

    fn copy_id(&mut self, label: &str, id: &str) {
        match self.environment.clipboard.set_text(id) {
            Ok(()) => self.status = StatusLine::info(format!("Copied {label} {id}")),
            Err(error) => self.report_error(&format!("Could not copy {label} {id}"), &error),
        }
    }
}
