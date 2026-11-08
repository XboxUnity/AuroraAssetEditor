use std::collections::HashMap;

use aurora_console::{Game, GameFilter, GameIds};

use super::{GameColumn, PresenceCell};

/// The console's games as the table shows them: filtered, then sorted, with one row selected at most.
/// Without a sort the rows keep the order of the console's database.
#[derive(Debug, Default)]
pub struct GameTable {
    games: Vec<Game>,
    filter: GameFilter,
    sort: Option<(GameColumn, bool)>,
    /// An index into `games`, so sorting and filtering do not move the selection.
    selected: Option<usize>,
    presence: HashMap<GameIds, PresenceCell>,
}

impl GameTable {
    /// Replaces the games; the selection and the presence cells belong to the old ones and go.
    pub fn set_games(&mut self, games: Vec<Game>) {
        self.clear();
        self.games = games;
    }

    pub fn clear(&mut self) {
        self.games.clear();
        self.selected = None;
        self.presence.clear();
    }

    /// The games that pass the filter, in the sorted order.
    pub fn visible(&self) -> Vec<&Game> {
        self.visible_indices()
            .into_iter()
            .map(|index| &self.games[index])
            .collect()
    }

    /// Every game's IDs, the visible games first in table order, so the rows on screen fill in first.
    pub fn ids_in_presence_order(&self) -> Vec<GameIds> {
        let visible = self.visible_indices();
        let hidden: Vec<usize> = (0..self.games.len())
            .filter(|index| !visible.contains(index))
            .collect();
        visible
            .into_iter()
            .chain(hidden)
            .map(|index| self.games[index].ids.clone())
            .collect()
    }

    /// A selected game that the new filter hides is deselected.
    pub fn set_filter(&mut self, filter: GameFilter) {
        self.filter = filter;
        let still_visible = self
            .selected
            .is_some_and(|selected| self.visible_indices().contains(&selected));
        if !still_visible {
            self.selected = None;
        }
    }

    pub fn filter(&self) -> &GameFilter {
        &self.filter
    }

    /// The first click on a column sorts it ascending, the next ones toggle the direction.
    pub fn sort_by(&mut self, column: GameColumn) {
        let ascending = match self.sort {
            Some((sorted, ascending)) if sorted == column => !ascending,
            _ => true,
        };
        self.sort = Some((column, ascending));
    }

    /// The sorted column and whether it is ascending.
    pub fn sort(&self) -> Option<(GameColumn, bool)> {
        self.sort
    }

    pub fn select_visible_row(&mut self, row: usize) -> Option<&Game> {
        let index = *self.visible_indices().get(row)?;
        self.selected = Some(index);
        self.games.get(index)
    }

    pub fn selected(&self) -> Option<&Game> {
        self.games.get(self.selected?)
    }

    /// Where the selected game is among the visible rows; `None` when none is selected or the filter hides it.
    pub fn selected_visible_row(&self) -> Option<usize> {
        let selected = self.selected?;
        self.visible_indices()
            .iter()
            .position(|index| *index == selected)
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    pub fn presence(&self, ids: &GameIds) -> PresenceCell {
        self.presence
            .get(ids)
            .copied()
            .unwrap_or(PresenceCell::Unknown)
    }

    pub fn contains(&self, ids: &GameIds) -> bool {
        self.game(ids).is_some()
    }

    /// The listed game with `ids`, if the list has it.
    pub fn game(&self, ids: &GameIds) -> Option<&Game> {
        self.games.iter().find(|game| &game.ids == ids)
    }

    pub fn set_presence(&mut self, ids: &GameIds, cell: PresenceCell) {
        self.presence.insert(ids.clone(), cell);
    }

    fn visible_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.games.len())
            .filter(|index| self.filter.matches(&self.games[*index]))
            .collect();
        if let Some((column, ascending)) = self.sort {
            indices.sort_by(|first, second| {
                let order = column.compare(&self.games[*first], &self.games[*second]);
                if ascending { order } else { order.reverse() }
            });
        }
        indices
    }
}
