/// What the Online page's search box holds: a TitleID, or free keywords (XboxUnity only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    TitleId,
    Keywords,
}
