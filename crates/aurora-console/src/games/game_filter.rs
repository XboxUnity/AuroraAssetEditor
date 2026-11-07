use super::Game;

/// Mirrors `FtpAssetsControl.FiltersChanged`: blank filters are ignored, the others are ANDed
/// case-insensitive substring matches on the untrimmed text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameFilter {
    pub title: String,
    pub title_id: String,
}

impl GameFilter {
    pub fn matches(&self, game: &Game) -> bool {
        contains_unless_blank(&game.title_name, &self.title)
            && contains_unless_blank(game.ids.title_id.as_str(), &self.title_id)
    }
}

fn contains_unless_blank(text: &str, filter: &str) -> bool {
    filter.trim().is_empty() || text.to_lowercase().contains(&filter.to_lowercase())
}
