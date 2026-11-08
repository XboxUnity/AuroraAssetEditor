use aurora_asset::AssetKind;

use crate::jobs::DownloadScope;

/// The "All" item or one kind of a console action's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindChoice {
    All,
    One(AssetKind),
}

impl KindChoice {
    /// The kinds the choice stands for, in the order console actions run them: GC, BK, GL, SS.
    pub fn kinds(self) -> Vec<AssetKind> {
        DownloadScope::from(self).kinds()
    }
}

impl From<KindChoice> for DownloadScope {
    fn from(choice: KindChoice) -> DownloadScope {
        match choice {
            KindChoice::All => DownloadScope::FullSet,
            KindChoice::One(kind) => DownloadScope::OneKind(kind),
        }
    }
}
