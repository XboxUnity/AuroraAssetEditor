use aurora_asset::AssetKind;

/// The "All" item or one kind of a console action's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindChoice {
    All,
    One(AssetKind),
}
