use aurora_asset::ScreenshotNumber;
use aurora_ui::ScreenshotTile;

use super::{KeptRows, ShownPicture};

/// The screenshot strip's thumbnails. A drag on one of them survives a render that does not change it.
pub type ScreenshotTiles = KeptRows<ScreenshotTileContent, ScreenshotTile>;

/// What one thumbnail shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenshotTileContent {
    pub number: ScreenshotNumber,
    pub picture: Option<ShownPicture>,
    pub filled: bool,
    pub selected: bool,
    pub is_next_free: bool,
}
