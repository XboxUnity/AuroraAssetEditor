use aurora_console::{GameIds, HexId, IdError};

use crate::text::filter_hex_input;

/// The "Save all assets to console" dialog's two boxes. Edits are filtered like the old ID boxes, and OK needs
/// eight hex digits in both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadTargetForm {
    pub title_id: String,
    pub database_id: String,
}

impl UploadTargetForm {
    /// Opens with the current game's IDs, or empty without one.
    pub fn new(game: Option<&GameIds>) -> UploadTargetForm {
        UploadTargetForm {
            title_id: game.map(|ids| ids.title_id.to_string()).unwrap_or_default(),
            database_id: game
                .map(|ids| ids.database_id.to_string())
                .unwrap_or_default(),
        }
    }

    pub fn edit_title_id(&mut self, text: &str) {
        self.title_id = filter_hex_input(text);
    }

    pub fn edit_database_id(&mut self, text: &str) {
        self.database_id = filter_hex_input(text);
    }

    pub fn is_valid(&self) -> bool {
        self.parse().is_ok()
    }

    pub fn parse(&self) -> Result<GameIds, IdError> {
        Ok(GameIds {
            title_id: HexId::parse(&self.title_id)?,
            database_id: HexId::parse(&self.database_id)?,
        })
    }
}
