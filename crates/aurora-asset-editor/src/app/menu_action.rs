use crate::settings::ThemeChoice;

use super::KindChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenAssetFiles,
    NewAssetSet,
    SaveCover,
    SaveBackground,
    SaveIconBanner,
    SaveScreenshots,
    SaveAllLocally,
    SaveAllToConsole,
    CopyTitleId,
    CopyDatabaseId,
    ClearSelection,
    ToggleAutoResize,
    ToggleUseCompression,
    Theme(ThemeChoice),
    /// The view shows the About dialog for it (`show-about`).
    About,
    Download(KindChoice),
    Upload(KindChoice),
    Remove(KindChoice),
}
