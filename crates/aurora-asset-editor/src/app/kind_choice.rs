use aurora_asset::AssetKind;

/// The "All" item or one kind of a console action's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindChoice {
    All,
    One(AssetKind),
}

impl KindChoice {
    /// The kinds the choice stands for, in the order console actions run them: GC, BK, GL, SS.
    pub fn kinds(self) -> Vec<AssetKind> {
        match self {
            KindChoice::All => AssetKind::ALL.to_vec(),
            KindChoice::One(kind) => vec![kind],
        }
    }
}
