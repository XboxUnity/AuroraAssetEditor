use super::HexId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameIds {
    pub title_id: HexId,
    pub database_id: HexId,
}

impl GameIds {
    pub fn folder_name(&self) -> String {
        format!("{}_{}", self.title_id, self.database_id)
    }
}
