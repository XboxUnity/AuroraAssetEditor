use std::cmp::Ordering;

use aurora_console::Game;

/// The game table's columns that can be sorted; the Assets column cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameColumn {
    Title,
    TitleId,
    DatabaseId,
    MediaId,
    Disc,
}

impl GameColumn {
    /// Titles compare ignoring case, discs as numbers and IDs as text.
    pub(super) fn compare(self, first: &Game, second: &Game) -> Ordering {
        match self {
            GameColumn::Title => first
                .title_name
                .to_lowercase()
                .cmp(&second.title_name.to_lowercase()),
            GameColumn::TitleId => first
                .ids
                .title_id
                .as_str()
                .cmp(second.ids.title_id.as_str()),
            GameColumn::DatabaseId => first
                .ids
                .database_id
                .as_str()
                .cmp(second.ids.database_id.as_str()),
            GameColumn::MediaId => first.media_id.as_str().cmp(second.media_id.as_str()),
            GameColumn::Disc => first.disc.cmp(&second.disc),
        }
    }
}
