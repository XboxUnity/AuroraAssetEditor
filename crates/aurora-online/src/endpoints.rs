/// Base URLs of the two cover services, each ending in `/` so a search term can be appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub xboxunity_covers: String,
    pub archive_covers: String,
}

impl Endpoints {
    /// The addresses the old app used.
    pub fn live() -> Endpoints {
        Endpoints {
            xboxunity_covers: "http://xboxunity.net/api/Covers/".to_string(),
            archive_covers: "https://archive.org/download/xboxunity-covers-fulldump_202311/xboxunity-covers-fulldump/".to_string(),
        }
    }
}
