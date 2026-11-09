use super::{App, CurrentGame};

const TITLE_ID_LABEL: &str = "TitleID";
const DATABASE_ID_LABEL: &str = "DatabaseID";

impl App {
    pub fn current_game(&self) -> Option<&CurrentGame> {
        self.current_game.as_ref()
    }

    pub fn set_current_game(&mut self, game: Option<CurrentGame>) {
        self.current_game = game;
        match &self.current_game {
            Some(game) => self.online.title_id_text = game.ids.title_id.to_string(),
            None => {
                self.online.title_id_text.clear();
                self.console.table.clear_selection();
            }
        }
    }

    pub fn clear_current_game(&mut self) {
        self.set_current_game(None);
    }

    pub fn copy_title_id(&mut self) {
        if let Some(game) = &self.current_game {
            let id = game.ids.title_id.to_string();
            self.copy_id(TITLE_ID_LABEL, &id);
        }
    }

    pub fn copy_database_id(&mut self) {
        if let Some(game) = &self.current_game {
            let id = game.ids.database_id.to_string();
            self.copy_id(DATABASE_ID_LABEL, &id);
        }
    }

    fn copy_id(&mut self, label: &str, id: &str) {
        match self.environment.clipboard.set_text(id) {
            Ok(()) => self.report_done(format!("Copied {label} {id}")),
            Err(error) => self.report_error(&format!("Could not copy {label} {id}"), &error),
        }
    }
}
