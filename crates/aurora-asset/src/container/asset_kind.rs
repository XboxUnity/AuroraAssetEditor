use super::AssetSlot;

/// The page of the app that owns a group of entries (GC, BK, GL, SS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Cover,
    Background,
    IconBanner,
    Screenshots,
}

impl AssetKind {
    pub const ALL: [AssetKind; 4] = [
        Self::Cover,
        Self::Background,
        Self::IconBanner,
        Self::Screenshots,
    ];

    /// The two letters Aurora puts in front of the title ID in an asset file name.
    pub fn file_prefix(self) -> &'static str {
        match self {
            Self::Cover => "GC",
            Self::Background => "BK",
            Self::IconBanner => "GL",
            Self::Screenshots => "SS",
        }
    }

    pub fn slots(self) -> impl Iterator<Item = AssetSlot> {
        AssetSlot::all().filter(move |slot| self.owns(*slot))
    }

    pub(super) fn owns(self, slot: AssetSlot) -> bool {
        match self {
            Self::Cover => slot == AssetSlot::Cover,
            Self::Background => slot == AssetSlot::Background,
            Self::IconBanner => matches!(slot, AssetSlot::Icon | AssetSlot::Banner),
            Self::Screenshots => matches!(slot, AssetSlot::Screenshot(_)),
        }
    }
}
