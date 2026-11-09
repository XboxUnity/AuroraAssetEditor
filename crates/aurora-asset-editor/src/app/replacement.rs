use aurora_asset::AssetKind;
use aurora_console::GameIds;

use crate::files::LoadedFiles;
use crate::jobs::{AfterDownload, DownloadScope, KindDownload};

/// Work that replaces pages, held while the user decides whether their unsaved changes may go.
pub(super) enum Replacement {
    Load(LoadedFiles),
    Download {
        ids: GameIds,
        scope: DownloadScope,
        after: AfterDownload,
        results: Vec<KindDownload>,
    },
    NewAssetSet,
}

/// A replacement waiting for its Discard question, with the pages it replaces.
pub(super) struct PendingReplacement {
    pub(super) kinds: Vec<AssetKind>,
    pub(super) work: Replacement,
}
