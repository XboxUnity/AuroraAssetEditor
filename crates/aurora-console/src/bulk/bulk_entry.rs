use super::BulkOutcome;
use crate::games::Game;

#[derive(Debug)]
pub struct BulkEntry {
    pub game: Game,
    pub outcome: BulkOutcome,
}
