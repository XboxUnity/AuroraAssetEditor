use aurora_console::GameIds;

/// The game the top bar's chip shows, picked on the Console page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentGame {
    pub title: String,
    pub ids: GameIds,
}
