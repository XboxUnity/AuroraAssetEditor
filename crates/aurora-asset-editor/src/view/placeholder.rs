use aurora_asset::AssetSlot;
use aurora_ui::{PreviewArea, placeholder_art};

/// The art an empty slot shows, one for each kind of slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placeholder {
    Cover,
    Background,
    Icon,
    Banner,
    Screenshot,
}

impl Placeholder {
    /// `None` for the unused fourth entry, which no page shows.
    pub fn of_slot(slot: AssetSlot) -> Option<Placeholder> {
        match slot {
            AssetSlot::Cover => Some(Placeholder::Cover),
            AssetSlot::Background => Some(Placeholder::Background),
            AssetSlot::Icon => Some(Placeholder::Icon),
            AssetSlot::Banner => Some(Placeholder::Banner),
            AssetSlot::Screenshot(_) => Some(Placeholder::Screenshot),
            AssetSlot::Slot => None,
        }
    }

    pub fn area(self) -> PreviewArea {
        match self {
            Placeholder::Cover => PreviewArea::CoverPlaceholder,
            Placeholder::Background => PreviewArea::BackgroundPlaceholder,
            Placeholder::Icon => PreviewArea::IconPlaceholder,
            Placeholder::Banner => PreviewArea::BannerPlaceholder,
            Placeholder::Screenshot => PreviewArea::ScreenshotPlaceholder,
        }
    }

    /// The PNG file of the art.
    pub fn png(self) -> &'static [u8] {
        match self {
            Placeholder::Cover => placeholder_art::COVER,
            Placeholder::Background => placeholder_art::BACKGROUND,
            Placeholder::Icon => placeholder_art::ICON,
            Placeholder::Banner => placeholder_art::BANNER,
            Placeholder::Screenshot => placeholder_art::SCREENSHOT,
        }
    }
}
