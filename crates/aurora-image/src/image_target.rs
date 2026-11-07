use aurora_asset::AssetSlot;

/// The fixed pixel sizes Aurora expects; an image of any other size is not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTarget {
    Cover,
    Background,
    Icon,
    Banner,
    Screenshot,
}

impl ImageTarget {
    pub fn size(self) -> (u32, u32) {
        match self {
            ImageTarget::Cover => (900, 600),
            ImageTarget::Background => (1280, 720),
            ImageTarget::Icon => (64, 64),
            ImageTarget::Banner => (420, 96),
            ImageTarget::Screenshot => (1000, 562),
        }
    }

    /// `None` for the unused fourth entry, which holds no picture.
    pub fn for_slot(slot: AssetSlot) -> Option<ImageTarget> {
        match slot {
            AssetSlot::Cover => Some(ImageTarget::Cover),
            AssetSlot::Background => Some(ImageTarget::Background),
            AssetSlot::Icon => Some(ImageTarget::Icon),
            AssetSlot::Banner => Some(ImageTarget::Banner),
            AssetSlot::Screenshot(_) => Some(ImageTarget::Screenshot),
            AssetSlot::Slot => None,
        }
    }
}
