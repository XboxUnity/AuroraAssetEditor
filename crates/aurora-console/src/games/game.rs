use crate::ids::{GameIds, HexId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub title_name: String,
    pub ids: GameIds,
    pub media_id: HexId,
    pub disc: u32,
}
