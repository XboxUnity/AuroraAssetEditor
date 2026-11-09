use aurora_asset::AssetKind;
use aurora_console::GameIds;

use crate::jobs::AssetUpload;

/// What a confirmation dialog does when it is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmedAction {
    RemoveFromConsole {
        ids: GameIds,
        kinds: Vec<AssetKind>,
    },
    /// An upload that includes empty pages; `files` holds the bytes as they were when the question was asked.
    UploadWithEmpty {
        ids: GameIds,
        files: Vec<AssetUpload>,
    },
    /// Discards unsaved changes: the work waiting to replace those pages goes ahead.
    DiscardChanges,
    /// Discards unsaved changes and closes the window.
    CloseWindow,
}
