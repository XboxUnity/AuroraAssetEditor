use std::rc::Rc;

use aurora_asset::ScreenshotNumber;
use aurora_ui::ScreenshotTile;
use slint::{Model, ModelRc, VecModel};

/// The screenshot strip's thumbnails, in one model the window keeps. A new model would make the strip build
/// every thumbnail anew, which ends a drag in progress; a row changed in place keeps its thumbnail. A render
/// changes only the rows whose content changed.
#[derive(Default)]
pub struct ScreenshotTiles {
    model: Rc<VecModel<ScreenshotTile>>,
    shown: Vec<TileContent>,
}

/// What one tile shows. The image is named by its slot's revision, so comparing tiles never compares pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileContent {
    pub number: ScreenshotNumber,
    pub image_revision: u64,
    pub filled: bool,
    pub selected: bool,
    pub is_next_free: bool,
}

impl ScreenshotTiles {
    /// The model the window's strip shows, for as long as the window lives.
    pub fn model(&self) -> ModelRc<ScreenshotTile> {
        ModelRc::from(Rc::clone(&self.model))
    }

    /// Shows `contents`; `make_tile` builds the tile for each one that differs from what the strip shows.
    pub fn show(
        &mut self,
        contents: Vec<TileContent>,
        mut make_tile: impl FnMut(&TileContent) -> ScreenshotTile,
    ) {
        for (row, content) in contents.iter().enumerate() {
            match self.shown.get(row) {
                Some(shown) if shown == content => {}
                Some(_) => self.model.set_row_data(row, make_tile(content)),
                None => self.model.push(make_tile(content)),
            }
        }
        for row in (contents.len()..self.shown.len()).rev() {
            self.model.remove(row);
        }
        self.shown = contents;
    }
}
