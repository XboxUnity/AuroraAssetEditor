use aurora_console::{Game, GameFilter};

/// The console's games in an ID dialog: the text that filters them and the match the arrow keys moved to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GamePicker {
    pub filter: String,
    /// An index into the matches. Typing highlights the first match, so Return picks it; with nothing
    /// highlighted, Return in the box is the dialog's OK.
    highlighted: Option<usize>,
}

impl GamePicker {
    /// The games whose title or TitleID holds the filter, ignoring case; all of them while it is blank.
    pub fn matches<'a>(&self, games: &[&'a Game]) -> Vec<&'a Game> {
        let by_title = GameFilter {
            title: self.filter.clone(),
            title_id: String::new(),
        };
        let by_title_id = GameFilter {
            title: String::new(),
            title_id: self.filter.clone(),
        };
        games
            .iter()
            .copied()
            .filter(|game| by_title.matches(game) || by_title_id.matches(game))
            .collect()
    }

    pub fn edit_filter(&mut self, text: &str) {
        self.filter = text.to_owned();
        self.highlighted = if text.trim().is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// The highlighted match among `match_count`; `None` when nothing is highlighted or nothing matches.
    pub fn highlighted(&self, match_count: usize) -> Option<usize> {
        let last = match_count.checked_sub(1)?;
        self.highlighted.map(|highlighted| highlighted.min(last))
    }

    /// Up and Down: moves the highlight by `step` matches, stopping at the first and last one. Without a
    /// highlight the first match is highlighted.
    pub fn move_highlight(&mut self, step: isize, match_count: usize) {
        let Some(last) = match_count.checked_sub(1) else {
            return;
        };
        self.highlighted = Some(match self.highlighted(match_count) {
            Some(row) => row.saturating_add_signed(step).min(last),
            None => 0,
        });
    }

    /// After a pick the next Return is the dialog's OK again.
    pub fn clear_highlight(&mut self) {
        self.highlighted = None;
    }
}
