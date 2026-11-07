use crate::ids::{GameIds, HexId};
use aurora_asset::AssetKind;

pub const GAME_DATA_DIR: &str = "/Game/Data/GameData";
pub const DATABASES_DIR: &str = "/Game/Data/DataBases";
pub const CONTENT_DB_FILE: &str = "Content.db";

pub fn asset_folder(ids: &GameIds) -> String {
    format!("{GAME_DATA_DIR}/{}", ids.folder_name())
}

pub fn asset_file_name(kind: AssetKind, title_id: &HexId) -> String {
    format!("{}{title_id}.asset", kind.file_prefix())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(title_id: &str, database_id: &str) -> GameIds {
        GameIds {
            title_id: HexId::parse(title_id).unwrap(),
            database_id: HexId::parse(database_id).unwrap(),
        }
    }

    #[test]
    fn asset_paths_match_old_app() {
        let ids = ids("4D5307E6", "00000012");
        assert_eq!(ids.folder_name(), "4D5307E6_00000012");
        assert_eq!(asset_folder(&ids), "/Game/Data/GameData/4D5307E6_00000012");
        assert_eq!(
            asset_file_name(AssetKind::Screenshots, &ids.title_id),
            "SS4D5307E6.asset"
        );
    }

    #[test]
    fn asset_paths_keep_typed_case() {
        let ids = ids("4d5307e6", "0000001a");
        assert_eq!(asset_folder(&ids), "/Game/Data/GameData/4d5307e6_0000001a");
        assert_eq!(
            asset_file_name(AssetKind::Cover, &ids.title_id),
            "GC4d5307e6.asset"
        );
    }
}
