use crate::games::Game;

/// Reported before each game is handled.
#[derive(Debug, Clone, Copy)]
pub struct BulkProgress<'a> {
    /// Counts from 1.
    pub position: usize,
    pub total: usize,
    pub game: &'a Game,
}
