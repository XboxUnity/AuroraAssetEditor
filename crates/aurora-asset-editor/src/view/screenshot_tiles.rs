use aurora_asset::ScreenshotNumber;
use aurora_ui::ScreenshotTile;

use super::KeptRows;

/// The screenshot strip's thumbnails. A drag on one of them survives a render that does not change it.
pub type ScreenshotTiles = KeptRows<ScreenshotTileContent, ScreenshotTile>;

/// What one thumbnail shows, with its image named by the slot's revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenshotTileContent {
    pub number: ScreenshotNumber,
    pub image_revision: u64,
    pub filled: bool,
    pub selected: bool,
    pub is_next_free: bool,
}
