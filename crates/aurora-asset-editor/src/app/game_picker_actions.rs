use aurora_console::Game;

use crate::console::RefreshOrigin;
use crate::dialogs::GamePicker;

use super::{App, PickerContent};

impl App {
    /// What the picker of the dialog on screen shows. `None` when the dialog has no picker, no console is set
    /// up, or the console listed no games: the dialog then shows only its ID boxes.
    pub fn game_picker_content(&self) -> Option<PickerContent<'_>> {
        let picker = self.modals.current()?.game_picker()?;
        self.console.settings.as_ref()?;
        if self.console.is_refreshing() {
            return Some(PickerContent::Loading);
        }
        let games = self.console.table.sorted_games();
        if games.is_empty() {
            return None;
        }
        let matches = picker.matches(&games);
        let highlighted = picker.highlighted(matches.len());
        Some(PickerContent::Games {
            matches,
            highlighted,
        })
    }

    pub fn edit_picker_filter(&mut self, text: &str) {
        if let Some(picker) = self.picker_mut() {
            picker.edit_filter(text);
        }
    }

    /// Up and Down in the picker's box.
    pub fn move_picker_highlight(&mut self, step: isize) {
        let match_count = self.picker_matches().len();
        if let Some(picker) = self.picker_mut() {
            picker.move_highlight(step, match_count);
        }
    }

    /// A click on a listed game fills the dialog's ID boxes with its IDs. The current game stays as it is.
    pub fn pick_game(&mut self, match_index: usize) {
        let Some(ids) = self
            .picker_matches()
            .get(match_index)
            .map(|game| game.ids.clone())
        else {
            log::error!("Game {match_index} of the picker was picked, which it does not list");
            return;
        };
        if let Some(modal) = self.modals.current_mut() {
            modal.take_game_ids(&ids);
        }
        if let Some(picker) = self.picker_mut() {
            picker.clear_highlight();
        }
    }

    /// Return in the picker's box picks the highlighted game.
    pub fn pick_highlighted_game(&mut self) {
        let match_count = self.picker_matches().len();
        let highlighted = self
            .modals
            .current()
            .and_then(|modal| modal.game_picker())
            .and_then(|picker| picker.highlighted(match_count));
        if let Some(index) = highlighted {
            self.pick_game(index);
        }
    }

    /// Opening a dialog with a picker lists the console's games when this session has not listed them yet,
    /// as the Console page's Refresh does, except that a connection failure is shown beside the dialog
    /// instead of in a dialog of its own.
    pub(super) fn load_games_for_picker(&mut self) {
        let listed_or_listing = self.console.last_refresh.is_some() || self.console.is_refreshing();
        if self.console.settings.is_none() || listed_or_listing {
            return;
        }
        self.list_games(RefreshOrigin::GamePicker);
    }

    fn picker_matches(&self) -> Vec<&Game> {
        let Some(picker) = self.modals.current().and_then(|modal| modal.game_picker()) else {
            return Vec::new();
        };
        picker.matches(&self.console.table.sorted_games())
    }

    fn picker_mut(&mut self) -> Option<&mut GamePicker> {
        self.modals.current_mut()?.game_picker_mut()
    }
}
