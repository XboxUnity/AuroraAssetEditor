use aurora_console::Game;

/// What the game picker of the ID dialog on screen shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerContent<'a> {
    /// The console's games are being listed; the dialog's own boxes work meanwhile.
    Loading,
    /// The games that match the filter, in the console table's order, and the highlighted one.
    Games {
        matches: Vec<&'a Game>,
        highlighted: Option<usize>,
    },
}
