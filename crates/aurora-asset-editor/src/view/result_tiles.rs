use aurora_ui::ResultTileView;
use slint::SharedString;

use super::{KeptRows, ShownPicture};

/// The Online page's result grid. A click on a result survives a render that does not change it.
pub type ResultTiles = KeptRows<ResultTileContent, ResultTileView>;

/// What one result shows: its thumbnail once it has loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultTileContent {
    pub label: SharedString,
    pub thumbnail: Option<ShownPicture>,
    pub failed: bool,
    pub selected: bool,
}
