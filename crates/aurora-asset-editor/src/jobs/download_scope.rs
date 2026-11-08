use aurora_asset::AssetKind;

/// What a console download fetches, which decides how the editor takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadScope {
    /// One kind: its page takes what arrived; a kind the console does not have leaves the page as it is.
    OneKind(AssetKind),
    /// Every kind: once all of them arrived, the pages become the console's set, a kind the console does not
    /// have an empty page. A failure changes no page.
    FullSet,
}

impl DownloadScope {
    /// The kinds to fetch, in the order the worker fetches them: GC, BK, GL, SS.
    pub fn kinds(self) -> Vec<AssetKind> {
        match self {
            DownloadScope::OneKind(kind) => vec![kind],
            DownloadScope::FullSet => AssetKind::ALL.to_vec(),
        }
    }
}
