use aurora_ui::ResultTileView;
use slint::SharedString;

use super::{KeptRows, ResultImageKey};

/// The Online page's result grid. A click on a result survives a render that does not change it.
pub type ResultTiles = KeptRows<ResultTileContent, ResultTileView>;

/// What one result shows, with its thumbnail named by its image cache key once it has loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultTileContent {
    pub label: SharedString,
    pub thumbnail: Option<ResultImageKey>,
    pub failed: bool,
    pub selected: bool,
}
